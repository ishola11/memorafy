use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::time::{interval, Duration as TokioDuration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::client::{CloudCollection, CloudItem, CloudItemCollection};
use super::{EnsureSessionError, SyncEngine};

const IDLE_POLL: Duration = Duration::from_secs(5);
const MIN_RECONNECT: Duration = Duration::from_secs(5);
const MAX_RECONNECT: Duration = Duration::from_secs(60);

enum RealtimeFailure {
    AuthExpired,
    NotLoggedIn,
    Transient(String),
}

impl From<EnsureSessionError> for RealtimeFailure {
    fn from(err: EnsureSessionError) -> Self {
        match err {
            EnsureSessionError::AuthExpired => RealtimeFailure::AuthExpired,
            EnsureSessionError::NotLoggedIn => RealtimeFailure::NotLoggedIn,
            EnsureSessionError::NotConfigured => {
                RealtimeFailure::Transient("Supabase not configured".to_string())
            }
            EnsureSessionError::Transient(msg) => RealtimeFailure::Transient(msg),
        }
    }
}

pub async fn run_realtime_loop(engine: Arc<SyncEngine>) {
    let mut reconnect_delay = MIN_RECONNECT;
    let mut auth_notice_logged = false;

    loop {
        if super::auth::load_session(engine.db()).ok().flatten().is_none() {
            reconnect_delay = MIN_RECONNECT;
            auth_notice_logged = false;
            tokio::time::sleep(IDLE_POLL).await;
            continue;
        }

        match run_session(engine.clone()).await {
            Ok(()) => {
                reconnect_delay = MIN_RECONNECT;
                tracing::debug!("realtime session ended");
            }
            Err(failure) => match failure {
                RealtimeFailure::AuthExpired => {
                    if !auth_notice_logged {
                        tracing::warn!(
                            "realtime stopped: session expired — sign in again to resume live sync"
                        );
                        auth_notice_logged = true;
                    }
                    reconnect_delay = MAX_RECONNECT;
                }
                RealtimeFailure::NotLoggedIn => {
                    reconnect_delay = MIN_RECONNECT;
                }
                RealtimeFailure::Transient(msg) => {
                    tracing::warn!("realtime disconnected: {msg}");
                    reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT);
                }
            },
        }

        tokio::time::sleep(reconnect_delay).await;
    }
}

const REALTIME_TOPIC: &str = "realtime:public:memora";
/// Renew the socket's JWT this long before it expires. Without renewal,
/// Supabase silently stops delivering rows once the join token expires
/// (~1 hour) even though the socket stays open.
const TOKEN_RENEWAL_MARGIN_SECS: i64 = 120;

fn seconds_until_renewal(expires_at: i64) -> u64 {
    let now = chrono::Utc::now().timestamp();
    (expires_at - TOKEN_RENEWAL_MARGIN_SECS - now).max(30) as u64
}

async fn run_session(engine: Arc<SyncEngine>) -> Result<(), RealtimeFailure> {
    let config = engine.config().ok_or(EnsureSessionError::NotConfigured)?;
    let mut session = super::ensure_session(&engine).await?;

    let ws_url = format!(
        "{}?apikey={}&vsn=1.0.0",
        config.realtime_url(),
        urlencoding::encode(&config.anon_key)
    );

    let (ws, _) = connect_async(&ws_url)
        .await
        .map_err(|e| RealtimeFailure::Transient(e.to_string()))?;
    let (mut write, mut read) = ws.split();

    let join = serde_json::json!({
        "topic": REALTIME_TOPIC,
        "event": "phx_join",
        "payload": {
            "config": {
                "broadcast": { "self": false },
                "presence": { "key": "" },
                "postgres_changes": [
                    {
                        "event": "*",
                        "schema": "public",
                        "table": "items"
                    },
                    {
                        "event": "*",
                        "schema": "public",
                        "table": "collections"
                    },
                    {
                        "event": "*",
                        "schema": "public",
                        "table": "item_collections"
                    }
                ]
            },
            "access_token": session.access_token
        },
        "ref": "1"
    });
    write
        .send(Message::Text(join.to_string().into()))
        .await
        .map_err(|e| RealtimeFailure::Transient(e.to_string()))?;

    tracing::info!("realtime connected");

    // Catch up on anything that changed while the socket was down
    // (disconnects, sleep/wake, first connect after being offline).
    if let Err(e) = engine.pull_incremental(&session).await {
        tracing::warn!("catch-up pull after realtime connect: {e}");
    }

    let mut heartbeat = interval(TokioDuration::from_secs(25));

    loop {
        let renewal_sleep =
            tokio::time::sleep(TokioDuration::from_secs(seconds_until_renewal(session.expires_at)));
        tokio::pin!(renewal_sleep);

        tokio::select! {
            _ = heartbeat.tick() => {
                let ping = serde_json::json!({
                    "topic": "phoenix",
                    "event": "heartbeat",
                    "payload": {},
                    "ref": "hb"
                });
                write
                    .send(Message::Text(ping.to_string().into()))
                    .await
                    .map_err(|e| RealtimeFailure::Transient(e.to_string()))?;
            }
            _ = &mut renewal_sleep => {
                // Refresh the JWT and hand the new token to the channel so
                // postgres_changes delivery survives past token expiry.
                session = super::ensure_session(&engine).await?;
                let renew = serde_json::json!({
                    "topic": REALTIME_TOPIC,
                    "event": "access_token",
                    "payload": { "access_token": session.access_token },
                    "ref": "at"
                });
                write
                    .send(Message::Text(renew.to_string().into()))
                    .await
                    .map_err(|e| RealtimeFailure::Transient(e.to_string()))?;
                tracing::debug!("realtime access token renewed");
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            if value.get("event").and_then(|v| v.as_str()) == Some("postgres_changes") {
                                let event_type = value
                                    .pointer("/payload/data/type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("INSERT");
                                let table = value
                                    .pointer("/payload/data/table")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("items");

                                if let Some(record) = value.pointer("/payload/data/record") {
                                    match table {
                                        "items" => {
                                            if let Ok(item) = serde_json::from_value::<CloudItem>(record.clone()) {
                                                let _ = engine.handle_remote_item(item).await;
                                            }
                                        }
                                        "collections" => {
                                            if let Ok(collection) = serde_json::from_value::<CloudCollection>(record.clone()) {
                                                let _ = engine.handle_remote_collection(collection, event_type).await;
                                            }
                                        }
                                        "item_collections" => {
                                            if let Ok(link) = serde_json::from_value::<CloudItemCollection>(record.clone()) {
                                                let _ = engine.handle_remote_item_collection(link, event_type).await;
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(RealtimeFailure::Transient(e.to_string())),
                    None => break,
                }
            }
        }
    }

    Ok(())
}
