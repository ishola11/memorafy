use std::collections::HashMap;

use super::auth::{claims_from_access_token, AuthSession};

/// Redirect target for Supabase email links (signup confirm, password reset, etc.).
/// Must be allowlisted in the Supabase dashboard → Authentication → URL Configuration.
pub const AUTH_REDIRECT_URL: &str = "memorafy://auth/callback";

fn parse_query_string(raw: &str) -> HashMap<String, String> {
    url::form_urlencoded::parse(raw.as_bytes())
        .into_owned()
        .collect()
}

/// Parse `memorafy://auth/callback#access_token=…` (or `?error=…`) from a deep link.
pub fn parse_auth_callback_url(url: &str) -> Result<(HashMap<String, String>, String), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("Invalid callback URL: {e}"))?;
    if parsed.scheme() != "memorafy" {
        return Err("Not a Memorafy auth callback".into());
    }

    let params = if let Some(fragment) = parsed.fragment() {
        parse_query_string(fragment)
    } else if parsed.query().is_some_and(|q| !q.is_empty()) {
        parse_query_string(parsed.query().unwrap_or_default())
    } else {
        return Err("Auth callback is missing tokens".into());
    };

    let callback_type = params
        .get("type")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());

    Ok((params, callback_type))
}

pub fn session_from_callback_tokens(
    access_token: &str,
    refresh_token: &str,
    expires_in: Option<i64>,
) -> Result<AuthSession, String> {
    let (user_id, _) = claims_from_access_token(access_token)?;
    let expires_at = chrono::Utc::now().timestamp() + expires_in.unwrap_or(3600);

    Ok(AuthSession {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.to_string(),
        user_id,
        expires_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hash_fragment_tokens() {
        let url = "memorafy://auth/callback#access_token=abc&refresh_token=def&expires_in=3600&type=signup";
        let (params, kind) = parse_auth_callback_url(url).unwrap();
        assert_eq!(params.get("access_token").map(String::as_str), Some("abc"));
        assert_eq!(params.get("refresh_token").map(String::as_str), Some("def"));
        assert_eq!(kind, "signup");
    }

    #[test]
    fn parses_error_query() {
        let url = "memorafy://auth/callback?error=access_denied&error_description=Expired";
        let (params, _) = parse_auth_callback_url(url).unwrap();
        assert_eq!(params.get("error").map(String::as_str), Some("access_denied"));
    }
}
