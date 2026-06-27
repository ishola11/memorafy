use chrono::{DateTime, Duration, Local, Utc};

use crate::db::{item_to_preview, ItemRecord, PreviewCardDto, TimelineSectionDto};

/// Time-bucket timeline for the History tab (excludes pinned/snippets).
pub fn build_history_timeline(items: &[ItemRecord]) -> Vec<TimelineSectionDto> {
    let now = Utc::now();
    let local_now = Local::now();

    let mut now_items = Vec::new();
    let mut today = Vec::new();
    let mut yesterday = Vec::new();
    let mut last_7 = Vec::new();
    let mut earlier = Vec::new();

    for item in items {
        let card = item_to_preview(item);
        let Ok(ts) = DateTime::parse_from_rfc3339(&item.created_at) else {
            earlier.push(card);
            continue;
        };
        let ts = ts.with_timezone(&Utc);
        let age = now.signed_duration_since(ts);

        if age < Duration::minutes(5) {
            now_items.push(card);
        } else if ts.date_naive() == local_now.date_naive() {
            today.push(card);
        } else if ts.date_naive() == local_now.date_naive() - Duration::days(1) {
            yesterday.push(card);
        } else if age < Duration::days(7) {
            last_7.push(card);
        } else {
            earlier.push(card);
        }
    }

    let mut sections = Vec::new();
    push_section(&mut sections, "now", "Now", now_items);
    push_section(&mut sections, "today", "Today", today);
    push_section(&mut sections, "yesterday", "Yesterday", yesterday);
    push_section(&mut sections, "last_7_days", "Last 7 Days", last_7);
    push_section(&mut sections, "earlier", "Earlier", earlier);
    sections
}

pub fn build_timeline(items: &[ItemRecord]) -> Vec<TimelineSectionDto> {
    let now = Utc::now();
    let local_now = Local::now();

    let mut pinned = Vec::new();
    let mut snippets = Vec::new();
    let mut now_items = Vec::new();
    let mut today = Vec::new();
    let mut yesterday = Vec::new();
    let mut last_7 = Vec::new();
    let mut earlier = Vec::new();

    for item in items {
        let card = item_to_preview(item);
        if item.kind == "snippet" {
            snippets.push(card);
            continue;
        }
        if item.is_pinned {
            pinned.push(card);
            continue;
        }

        let Ok(ts) = DateTime::parse_from_rfc3339(&item.created_at) else {
            earlier.push(card);
            continue;
        };
        let ts = ts.with_timezone(&Utc);
        let age = now.signed_duration_since(ts);

        if age < Duration::minutes(5) {
            now_items.push(card);
        } else if ts.date_naive() == local_now.date_naive() {
            today.push(card);
        } else if ts.date_naive() == local_now.date_naive() - Duration::days(1) {
            yesterday.push(card);
        } else if age < Duration::days(7) {
            last_7.push(card);
        } else {
            earlier.push(card);
        }
    }

    let mut sections = Vec::new();
    push_section(&mut sections, "pinned", "Pinned", pinned);
    push_section(&mut sections, "snippets", "Snippets", snippets);
    push_section(&mut sections, "now", "Now", now_items);
    push_section(&mut sections, "today", "Today", today);
    push_section(&mut sections, "yesterday", "Yesterday", yesterday);
    push_section(&mut sections, "last_7_days", "Last 7 Days", last_7);
    push_section(&mut sections, "earlier", "Earlier", earlier);
    sections
}

fn push_section(
    sections: &mut Vec<TimelineSectionDto>,
    bucket: &str,
    label: &str,
    items: Vec<PreviewCardDto>,
) {
    if !items.is_empty() {
        sections.push(TimelineSectionDto {
            bucket: bucket.to_string(),
            label: label.to_string(),
            items,
        });
    }
}
