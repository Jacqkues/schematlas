//! The team channel's data model, independent of git: events, presence, the branch's file layout
//! and the reducer that turns the branch's files into what the window shows. Everything read from
//! the branch is untrusted and validated here.
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

pub const VERSION: u32 = 1;
const MAX_EVENT: usize = 8 * 1024;
const MAX_TEXT: usize = 4 * 1024;
pub const RETENTION_DAYS: i64 = 14;
/// Presence older than this is shown as away.
pub const PRESENCE_TTL_SECS: i64 = 5 * 60;
const SHOWN_EVENTS: usize = 500;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Author {
    pub key: String,
    pub name: String,
}
impl Author {
    /// The key is derived from the email so the address itself never lands on the branch.
    pub fn new(name: &str, email: &str) -> Self {
        let digest = Sha256::digest(email.trim().to_lowercase().as_bytes());
        Self {
            key: digest[..6].iter().map(|b| format!("{b:02x}")).collect(),
            name: name.trim().chars().take(80).collect(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: String,
    pub v: u32,
    pub author: Author,
    pub lamport: u64,
    pub ts: String,
    pub kind: String,
    #[serde(default)]
    pub body: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Presence {
    pub author: Author,
    pub ts: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagram: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What the window posts; the hub fills in id, author, clock and time.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub kind: String,
    #[serde(default)]
    pub body: Value,
}

fn text_field(body: &Value, field: &str, required: bool) -> Result<()> {
    match body.get(field) {
        Some(Value::String(s)) if !s.trim().is_empty() && s.len() <= MAX_TEXT => Ok(()),
        None | Some(Value::Null) if !required => Ok(()),
        _ => Err(AppError::Validation(format!(
            "`{field}` must be text of 1 to {} KB.",
            MAX_TEXT / 1024
        ))),
    }
}
/// Shared by drafts and events read from the branch.
pub fn validate_body(kind: &str, body: &Value) -> Result<()> {
    if !body.is_object() {
        return Err(AppError::Validation("Event bodies are objects.".into()));
    }
    match kind {
        "chat" => text_field(body, "text", true),
        "comment" => {
            text_field(body, "text", true)?;
            text_field(body, "diagram", true)?;
            text_field(body, "node", false)?;
            text_field(body, "replyTo", false)
        }
        "comment_resolve" => text_field(body, "commentId", true),
        "editing" => {
            text_field(body, "diagram", false)?;
            text_field(body, "note", false)
        }
        "editing_done" => Ok(()),
        "diagram_changed" => {
            text_field(body, "diagram", true)?;
            text_field(body, "branch", false)?;
            text_field(body, "summary", false)?;
            match body.get("commit").and_then(Value::as_str) {
                Some(c)
                    if (7..=64).contains(&c.len()) && c.chars().all(|c| c.is_ascii_hexdigit()) =>
                {
                    Ok(())
                }
                None => Ok(()),
                _ => Err(AppError::Validation(
                    "`commit` must be a commit hash.".into(),
                )),
            }
        }
        _ => Err(AppError::Validation(format!(
            "Unknown event kind `{kind}`."
        ))),
    }
}
impl Event {
    fn valid(&self) -> bool {
        self.v == VERSION
            && !self.id.is_empty()
            && self.id.len() <= 64
            && self.author.key.len() == 12
            && self.author.key.chars().all(|c| c.is_ascii_hexdigit())
            && chrono::DateTime::parse_from_rfc3339(&self.ts).is_ok()
            && validate_body(&self.kind, &self.body).is_ok()
    }
    pub fn line(&self) -> Result<String> {
        let line = serde_json::to_string(self)?;
        if line.len() > MAX_EVENT {
            return Err(AppError::Validation("This message is too long.".into()));
        }
        Ok(line)
    }
}

/// `events/<author>/<YYYY-MM-DD>.jsonl`
pub fn event_path(author: &Author, ts: &chrono::DateTime<chrono::Utc>) -> String {
    format!("events/{}/{}.jsonl", author.key, ts.format("%Y-%m-%d"))
}
pub fn presence_path(author: &Author) -> String {
    format!("presence/{}.json", author.key)
}
/// Day files past retention, and presence files a week stale, are dropped when anyone writes.
pub fn expired(path: &str, now: &chrono::DateTime<chrono::Utc>, presence_ts: Option<&str>) -> bool {
    if let Some(day) = path
        .strip_prefix("events/")
        .and_then(|p| p.rsplit('/').next())
        .and_then(|f| f.strip_suffix(".jsonl"))
    {
        return chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .is_ok_and(|d| (now.date_naive() - d).num_days() > RETENTION_DAYS);
    }
    if path.starts_with("presence/") {
        return presence_ts
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
            .is_none_or(|ts| (*now - ts.with_timezone(&chrono::Utc)).num_days() > 7);
    }
    false
}

/// The branch's content, folded into the ordered feed and live presence.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub events: BTreeMap<(u64, String, String, String), Event>,
    pub presence: HashMap<String, Presence>,
    pub lamport: u64,
}
impl State {
    /// Replace everything from a full read of the branch tree (path → content).
    pub fn from_files(files: &[(String, String)]) -> Self {
        let mut state = State::default();
        for (path, content) in files {
            if path.starts_with("events/") && path.ends_with(".jsonl") {
                let owner = path.split('/').nth(1).unwrap_or_default();
                for line in content
                    .lines()
                    .filter(|l| !l.trim().is_empty() && l.len() <= MAX_EVENT)
                {
                    // An event only counts in its author's own folder, so nobody can post as someone else
                    // by writing into another folder.
                    if let Ok(event) = serde_json::from_str::<Event>(line) {
                        if event.valid() && event.author.key == owner {
                            state.add(event);
                        }
                    }
                }
            } else if let Some(key) = path
                .strip_prefix("presence/")
                .and_then(|p| p.strip_suffix(".json"))
            {
                if let Ok(presence) = serde_json::from_str::<Presence>(content) {
                    if presence.author.key == key
                        && chrono::DateTime::parse_from_rfc3339(&presence.ts).is_ok()
                    {
                        state.presence.insert(key.to_owned(), presence);
                    }
                }
            }
        }
        state
    }
    pub fn add(&mut self, event: Event) {
        self.lamport = self.lamport.max(event.lamport);
        if self.events.values().any(|e| e.id == event.id) {
            return;
        }
        self.events.insert(
            (
                event.lamport,
                event.ts.clone(),
                event.author.key.clone(),
                event.id.clone(),
            ),
            event,
        );
    }
    /// The most recent events, oldest first.
    pub fn feed(&self) -> Vec<Event> {
        let skip = self.events.len().saturating_sub(SHOWN_EVENTS);
        self.events.values().skip(skip).cloned().collect()
    }
    pub fn active(&self, now: &chrono::DateTime<chrono::Utc>) -> Vec<Presence> {
        let mut active: Vec<Presence> = self
            .presence
            .values()
            .filter(|p| {
                chrono::DateTime::parse_from_rfc3339(&p.ts).is_ok_and(|ts| {
                    (*now - ts.with_timezone(&chrono::Utc)).num_seconds() < PRESENCE_TTL_SECS
                })
            })
            .cloned()
            .collect();
        active.sort_by(|a, b| a.author.name.cmp(&b.author.name));
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(author: &Author, lamport: u64, id: &str, text: &str) -> Event {
        Event {
            id: id.into(),
            v: VERSION,
            author: author.clone(),
            lamport,
            ts: "2026-10-13T10:00:00Z".into(),
            kind: "chat".into(),
            body: json!({"text": text}),
        }
    }

    #[test]
    fn events_are_ordered_deduplicated_and_owned_by_their_folder() {
        let ana = Author::new("Ana", "ana@example.com");
        let bo = Author::new("Bo", "BO@example.com ");
        assert_eq!(bo.key, Author::new("Bo", "bo@example.com").key);
        let ana_file = [event(&ana, 3, "a2", "later"), event(&ana, 1, "a1", "first")]
            .iter()
            .map(|e| e.line().unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        // Bo's folder holds one valid event, one forged as Ana and one garbage line.
        let bo_file = format!(
            "{}\n{}\nnot json\n{}",
            event(&bo, 2, "b1", "middle").line().unwrap(),
            event(&ana, 9, "forged", "x").line().unwrap(),
            event(&bo, 2, "b1", "middle").line().unwrap()
        );
        let state = State::from_files(&[
            (format!("events/{}/2026-10-13.jsonl", ana.key), ana_file),
            (format!("events/{}/2026-10-13.jsonl", bo.key), bo_file),
        ]);
        let ids: Vec<_> = state.feed().iter().map(|e| e.id.clone()).collect();
        assert_eq!(ids, ["a1", "b1", "a2"]);
        assert_eq!(state.lamport, 3);
    }

    #[test]
    fn bodies_and_retention() {
        assert!(validate_body("chat", &json!({"text":"hi"})).is_ok());
        assert!(validate_body("chat", &json!({"text":""})).is_err());
        assert!(validate_body("comment", &json!({"text":"hi"})).is_err());
        assert!(validate_body("diagram_changed", &json!({"diagram":"x","commit":"zz"})).is_err());
        assert!(validate_body("rm -rf", &json!({})).is_err());
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-30T00:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert!(expired("events/abc/2026-10-01.jsonl", &now, None));
        assert!(!expired("events/abc/2026-10-20.jsonl", &now, None));
        assert!(expired(
            "presence/abc.json",
            &now,
            Some("2026-10-01T00:00:00Z")
        ));
        assert!(!expired("meta.json", &now, None));
    }
}
