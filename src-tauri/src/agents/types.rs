use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub status: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub details: Value,
    pub options: Vec<ReviewOption>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSnapshot {
    pub project_id: String,
    pub status: String,
    pub agent_name: String,
    pub session_id: Option<String>,
    pub messages: Vec<AgentMessage>,
    pub reviews: Vec<Review>,
    pub auth_methods: Vec<Value>,
    pub error: Option<String>,
    #[serde(default)]
    pub activity: String,
    #[serde(default)]
    pub last_activity_at: i64,
    #[serde(default)]
    pub turn_started_at: Option<i64>,
}
impl AgentSnapshot {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            status: "connecting".into(),
            agent_name: "Local agent".into(),
            session_id: None,
            messages: vec![],
            reviews: vec![],
            auth_methods: vec![],
            error: None,
            activity: "connecting".into(),
            last_activity_at: chrono::Utc::now().timestamp_millis(),
            turn_started_at: None,
        }
    }
    pub fn push(&mut self, role: &str, text: String) {
        self.messages.push(AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: role.into(),
            text: bounded(&text, 48 * 1024),
            status: None,
        });
        self.trim();
    }
    pub fn trim(&mut self) {
        while self.messages.len() > 200
            || (self.messages.len() > 1
                && self.messages.iter().map(|m| m.text.len()).sum::<usize>() > 512 * 1024)
        {
            self.messages.remove(0);
        }
    }
}
pub fn bounded(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.into();
    }
    let mut n = max;
    while !text.is_char_boundary(n) {
        n -= 1;
    }
    format!("{}\n[truncated]", &text[..n])
}
