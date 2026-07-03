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

        Self::build(embedded::SUPABASE_URL?, embedded::SUPABASE_ANON_KEY?)
    }

    fn from_runtime_env() -> Option<Self> {
        let url = env::var("SUPABASE_URL").ok()?;
        let anon_key = env::var("SUPABASE_ANON_KEY").ok()?;
        Self::build(&url, &anon_key)
    }

    /// Trims stray whitespace/newlines (common when secrets are pasted into
    /// env files or CI) so they can't later corrupt HTTP headers.
    fn build(url: &str, anon_key: &str) -> Option<Self> {
        let url = url.trim();
        let anon_key = anon_key.trim();
        if url.is_empty() || anon_key.is_empty() {
            return None;
        }
        Some(Self {
            url: url.to_string(),
            anon_key: anon_key.to_string(),
        })
    }

    pub fn storage_url(&self) -> String {
        format!("{}/storage/v1", self.url.trim_end_matches('/'))
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
