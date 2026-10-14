//! Display metadata stays with the harness; nothing here is written to the workspace.
use super::{
    session::Session,
    types::{bounded, Protocol},
};
use serde_json::{json, Value};
use std::{collections::HashMap, path::Path, sync::atomic::Ordering, time::Duration};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

fn title(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| bounded(&s.split_whitespace().collect::<Vec<_>>().join(" "), 240))
}

impl Session {
    pub(super) async fn remember_title(&self, id: &str, value: &Value) {
        let mut titles = self.titles.lock().await;
        if let Some(title) = title(value) {
            titles.insert(id.into(), title);
        } else {
            titles.remove(id);
        }
    }

    pub(super) async fn conversation_titles(&self, ids: &[String]) -> HashMap<String, String> {
        match self.config.protocol {
            Protocol::Codex => {
                // Metadata only: do not resume a thread or request its transcript.
                for id in ids {
                    if let Ok(result) = self
                        .rpc(
                            "thread/read",
                            json!({"threadId":id,"includeTurns":false}),
                            Duration::from_secs(3),
                        )
                        .await
                    {
                        let thread = &result["thread"];
                        // Codex's own display preview covers threads it hasn't named yet.
                        let value = if title(&thread["name"]).is_some() {
                            &thread["name"]
                        } else {
                            &thread["preview"]
                        };
                        self.remember_title(id, value).await;
                    }
                }
            }
            Protocol::Acp if self.lists_sessions.load(Ordering::Relaxed) => {
                let mut cursor = None::<String>;
                // Bound pagination, including broken agents that repeat a cursor.
                for _ in 0..20 {
                    let Ok(result) = self
                        .rpc(
                            "session/list",
                            json!({"cwd":self.config.cwd,"cursor":cursor}),
                            Duration::from_secs(3),
                        )
                        .await
                    else {
                        break;
                    };
                    if let Some(rows) = result["sessions"].as_array() {
                        for row in rows {
                            if let Some(id) = row["sessionId"]
                                .as_str()
                                .filter(|id| ids.iter().any(|saved| saved == id))
                            {
                                self.remember_title(id, &row["title"]).await;
                            }
                        }
                    }
                    let next = result["nextCursor"].as_str().map(str::to_owned);
                    if next.is_none() || next == cursor {
                        break;
                    }
                    cursor = next;
                }
            }
            _ => {}
        }
        self.titles.lock().await.clone()
    }
}

/// Matches Claude Agent SDK's bounded head/tail metadata reads, with title fields
/// restricted to top-level metadata records (never tool inputs or message text).
pub(super) async fn claude_title(root: &Path, cwd: &str, id: &str) -> Option<String> {
    uuid::Uuid::parse_str(id).ok()?;
    let sanitized: String = cwd
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let projects = root.join("projects");
    let mut directory = projects.join(&sanitized);
    if sanitized.len() > 200 {
        let mut entries = tokio::fs::read_dir(&projects).await.ok()?;
        while let Some(entry) = entries.next_entry().await.ok()? {
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(&format!("{}-", &sanitized[..200]))
                && tokio::fs::try_exists(entry.path().join(format!("{id}.jsonl")))
                    .await
                    .ok()?
            {
                directory = entry.path();
                break;
            }
        }
    }
    let mut file = tokio::fs::File::open(directory.join(format!("{id}.jsonl")))
        .await
        .ok()?;
    let size = file.metadata().await.ok()?.len();
    let mut chunks = vec![];
    for offset in [0, size.saturating_sub(65536)] {
        file.seek(std::io::SeekFrom::Start(offset)).await.ok()?;
        let mut bytes = Vec::new();
        (&mut file).take(65536).read_to_end(&mut bytes).await.ok()?;
        chunks.push(bytes);
    }
    let mut fields = HashMap::<String, String>::new();
    for bytes in chunks {
        for line in bytes.split(|b| *b == b'\n') {
            if let Ok(record) = serde_json::from_slice::<Value>(line) {
                for key in ["customTitle", "aiTitle", "summary"] {
                    if let Some(value) = title(&record[key]) {
                        fields.insert(key.into(), value);
                    }
                }
            }
        }
    }
    ["customTitle", "aiTitle", "summary"]
        .into_iter()
        .find_map(|key| fields.remove(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn claude_names_use_latest_metadata_and_ignore_tool_inputs() {
        let root = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let dir = root.path().join("projects/-example-repo");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{id}.jsonl")),
            concat!(
                "{\"type\":\"custom-title\",\"customTitle\":\"Old name\"}\n",
                "{\"type\":\"ai-title\",\"aiTitle\":\"Generated name\"}\n",
                "{\"type\":\"assistant\",\"message\":{\"customTitle\":\"Tool argument\"}}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"New name\"}\n"
            ),
        )
        .unwrap();
        assert_eq!(
            claude_title(root.path(), "/example/repo", &id)
                .await
                .as_deref(),
            Some("New name")
        );
        assert!(claude_title(root.path(), "/other/repo", &id)
            .await
            .is_none());
        assert!(claude_title(root.path(), "/example/repo", "../escape")
            .await
            .is_none());
    }
}
