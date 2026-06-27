mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_config.rs"));
}

use std::env;

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub url: String,
    pub anon_key: String,
}

impl SyncConfig {
    pub fn from_env() -> Option<Self> {
        if let Some(config) = Self::from_runtime_env() {
            return Some(config);
        }

        let url = embedded::SUPABASE_URL?;
        let anon_key = embedded::SUPABASE_ANON_KEY?;
        if url.is_empty() || anon_key.is_empty() {
            return None;
        }
        Some(Self {
            url: url.to_string(),
            anon_key: anon_key.to_string(),
        })
    }

    fn from_runtime_env() -> Option<Self> {
        let url = env::var("SUPABASE_URL").ok()?;
        let anon_key = env::var("SUPABASE_ANON_KEY").ok()?;
        if url.is_empty() || anon_key.is_empty() {
            return None;
        }
        Some(Self { url, anon_key })
    }

    pub fn rest_url(&self) -> String {
        format!("{}/rest/v1", self.url.trim_end_matches('/'))
    }

    pub fn auth_url(&self) -> String {
        format!("{}/auth/v1", self.url.trim_end_matches('/'))
    }

    pub fn realtime_url(&self) -> String {
        let host = self.url.trim_end_matches('/').replace("https://", "wss://");
        format!("{host}/realtime/v1/websocket")
    }
}
