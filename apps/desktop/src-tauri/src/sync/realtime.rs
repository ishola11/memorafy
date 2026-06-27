use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::time::{interval, Duration as TokioDuration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::client::CloudItem;
use super::SyncEngine;

pub async fn run_realtime_loop(engine: Arc<SyncEngine>) {
    loop {
        if super::auth::load_session(engine.db()).ok().flatten().is_none() {
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }
        if let Err(e) = run_session(engine.clone()).await {
            tracing::warn!("realtime disconnected: {e}");
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn run_session(engine: Arc<SyncEngine>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = engine.config().ok_or("not configured")?.clone();
    let session = super::ensure_session(&engine).await?;

    let ws_url = format!(
        "{}?apikey={}&vsn=1.0.0",
        config.realtime_url(),
        urlencoding::encode(&config.anon_key)
    );

    let (ws, _) = connect_async(&ws_url).await?;
    let (mut write, mut read) = ws.split();

    let join = serde_json::json!({
        "topic": "realtime:public:items",
        "event": "phx_join",
        "payload": {
            "config": {
                "broadcast": { "self": false },
                "presence": { "key": "" },
                "postgres_changes": [{
                    "event": "*",
                    "schema": "public",
                    "table": "items"
                }]
            },
            "access_token": session.access_token
        },
        "ref": "1"
    });
    write.send(Message::Text(join.to_string().into())).await?;

    let mut heartbeat = interval(TokioDuration::from_secs(25));

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let ping = serde_json::json!({
                    "topic": "phoenix",
                    "event": "heartbeat",
                    "payload": {},
                    "ref": "hb"
                });
                write.send(Message::Text(ping.to_string().into())).await?;
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                            if value.get("event").and_then(|v| v.as_str()) == Some("postgres_changes") {
                                if let Some(record) = value
                                    .pointer("/payload/data/record")
                                    .and_then(|r| serde_json::from_value::<CloudItem>(r.clone()).ok())
                                {
                                    let _ = engine.handle_remote_item(record).await;
                                }
                            }
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => return Err(e.into()),
                    None => break,
                }
            }
        }
    }

    Ok(())
}
