//! In-app feedback & issue reporting.
//!
//! Submission is provider-abstracted: the UI builds a [`FeedbackReport`],
//! and a [`FeedbackProvider`] decides where it goes. The current provider
//! drafts a prefilled GitHub issue in the user's browser — no server-side
//! component and nothing is transmitted until the user submits the issue
//! themselves. Swapping in an API-backed provider (GitHub REST, Supabase,
//! Discussions) only requires a new `FeedbackProvider` impl.

use serde::{Deserialize, Serialize};

/// GitHub repository that receives feedback issues.
const FEEDBACK_REPO: &str = "ishola11/memora";
/// Browsers reject very long URLs; keep the prefilled issue body well under
/// common limits. Anything truncated is still in the local logs.
const MAX_ISSUE_BODY_CHARS: usize = 6000;
/// Log lines offered in the diagnostics preview.
const DIAGNOSTIC_LOG_LINES: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsDto {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub sync_configured: bool,
    pub logged_in: bool,
    pub pending_count: i64,
    pub device_id: Option<String>,
    pub account_id: Option<String>,
    pub recent_logs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackReport {
    /// "bug" or "feature"
    pub kind: String,
    pub title: String,
    /// Bug: description / steps / expected / actual. Feature: idea / why / workflow.
    pub sections: Vec<FeedbackSection>,
    pub contact_email: Option<String>,
    /// Present only when the user explicitly consented in the UI.
    pub diagnostics: Option<DiagnosticsDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackSection {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum FeedbackOutcome {
    /// The report was rendered into a URL the caller should open externally.
    OpenUrl { url: String },
}

pub trait FeedbackProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn submit(&self, report: &FeedbackReport) -> Result<FeedbackOutcome, String>;
}

pub fn default_provider() -> Box<dyn FeedbackProvider> {
    Box::new(GitHubIssueDraft {
        repo: FEEDBACK_REPO.to_string(),
    })
}

/// Drafts a prefilled GitHub "new issue" page. The user reviews and submits
/// in their own browser under their own account — Memora sends nothing.
struct GitHubIssueDraft {
    repo: String,
}

impl FeedbackProvider for GitHubIssueDraft {
    fn name(&self) -> &'static str {
        "github-issue-draft"
    }

    fn submit(&self, report: &FeedbackReport) -> Result<FeedbackOutcome, String> {
        if report.title.trim().is_empty() {
            return Err("Please add a title before submitting.".to_string());
        }

        let label = if report.kind == "feature" {
            "enhancement"
        } else {
            "bug"
        };

        let mut body = render_markdown_body(report);
        if body.chars().count() > MAX_ISSUE_BODY_CHARS {
            body = body.chars().take(MAX_ISSUE_BODY_CHARS).collect::<String>()
                + "\n\n_(truncated to fit the issue draft)_";
        }

        let url = format!(
            "https://github.com/{}/issues/new?title={}&labels={}&body={}",
            self.repo,
            urlencoding::encode(report.title.trim()),
            label,
            urlencoding::encode(&body),
        );
        Ok(FeedbackOutcome::OpenUrl { url })
    }
}

fn render_markdown_body(report: &FeedbackReport) -> String {
    let mut body = String::new();

    for section in &report.sections {
        if section.value.trim().is_empty() {
            continue;
        }
        body.push_str(&format!("### {}\n\n{}\n\n", section.label, section.value.trim()));
    }

    if let Some(email) = report.contact_email.as_deref().filter(|e| !e.trim().is_empty()) {
        body.push_str(&format!("### Contact\n\n{}\n\n", email.trim()));
    }

    if let Some(diag) = &report.diagnostics {
        body.push_str("### Diagnostics\n\n");
        body.push_str(&format!(
            "| Field | Value |\n|---|---|\n\
             | App version | {} |\n\
             | OS | {} ({}) |\n\
             | Sync configured | {} |\n\
             | Signed in | {} |\n\
             | Pending changes | {} |\n",
            diag.app_version, diag.os, diag.arch, diag.sync_configured, diag.logged_in, diag.pending_count,
        ));
        if let Some(device) = &diag.device_id {
            body.push_str(&format!("| Device ID | {device} |\n"));
        }
        if let Some(account) = &diag.account_id {
            body.push_str(&format!("| Account ID | {account} |\n"));
        }
        if let Some(logs) = diag.recent_logs.as_deref().filter(|l| !l.is_empty()) {
            body.push_str(&format!("\n<details><summary>Recent logs</summary>\n\n```\n{logs}\n```\n\n</details>\n"));
        }
        body.push('\n');
    }

    body.push_str("---\n_Submitted from Memora's in-app feedback form._\n");
    body
}

pub fn collect_diagnostics(
    state: &crate::AppState,
    include_logs: bool,
) -> Result<DiagnosticsDto, String> {
    let sync_state = state.sync_engine.get_state()?;
    let account_id = state
        .db
        .get_setting(crate::db::SETTING_LAST_AUTH_USER_ID)
        .map_err(|e| e.to_string())?;

    Ok(DiagnosticsDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        sync_configured: sync_state.configured,
        logged_in: sync_state.logged_in,
        pending_count: sync_state.pending_count,
        device_id: Some(state.device_id()),
        account_id: if sync_state.logged_in { account_id } else { None },
        recent_logs: if include_logs {
            crate::logging::recent_log_tail(DIAGNOSTIC_LOG_LINES)
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_draft_builds_prefilled_url() {
        let provider = GitHubIssueDraft {
            repo: "owner/repo".to_string(),
        };
        let report = FeedbackReport {
            kind: "bug".to_string(),
            title: "Sync stalls after wake".to_string(),
            sections: vec![FeedbackSection {
                label: "Description".to_string(),
                value: "Items stop syncing".to_string(),
            }],
            contact_email: None,
            diagnostics: None,
        };
        let FeedbackOutcome::OpenUrl { url } = provider.submit(&report).expect("submit");
        assert!(url.starts_with("https://github.com/owner/repo/issues/new?"));
        assert!(url.contains("labels=bug"));
    }

    #[test]
    fn empty_title_is_rejected() {
        let provider = GitHubIssueDraft {
            repo: "owner/repo".to_string(),
        };
        let report = FeedbackReport {
            kind: "bug".to_string(),
            title: "  ".to_string(),
            sections: vec![],
            contact_email: None,
            diagnostics: None,
        };
        assert!(provider.submit(&report).is_err());
    }
}
