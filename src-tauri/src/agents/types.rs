use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    #[default]
    Acp,
    Codex,
    Claude,
}
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: String,
    #[serde(default)]
    pub protocol: Protocol,
}
impl AgentConfig {
    /// Identifies the harness that owns a session handle. No transcript or credentials.
    pub fn identity(&self) -> String {
        let path = std::path::Path::new(&self.executable)
            .canonicalize()
            .unwrap_or_else(|_| self.executable.clone().into());
        // Preserve existing ACP routing keys; native protocols have their own identity.
        let identity = if self.protocol == Protocol::Acp {
            serde_json::to_string(&(path, &self.args, &self.cwd))
        } else {
            serde_json::to_string(&(path, &self.args, &self.cwd, self.protocol))
        };
        crate::repo::hash(identity.unwrap_or_default().as_bytes())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    pub status: Option<String>,
    /// What a tool call returned, shown under its title. `text` stays the
    /// one-line summary so the collapsed row keeps its shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
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
/// How freely the harness may act on its own (edit files, run commands): one of the modes it
/// offers. Schematlas's own tools keep their in-app review in every mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMode {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentModel {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub resolved_id: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSnapshot {
    pub project_id: String,
    pub status: String,
    pub agent_name: String,
    #[serde(default)]
    pub executable: String,
    #[serde(default)]
    pub models: Vec<AgentModel>,
    #[serde(skip)]
    pub model_config_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub modes: Vec<AgentMode>,
    #[serde(skip)]
    pub mode_config_id: Option<String>,
    #[serde(default)]
    pub mode_id: Option<String>,
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
    /// True when these messages were replayed by `session/load` rather than
    /// produced in this session. The panel says so once, then clears it.
    #[serde(default)]
    pub resumed: bool,
    /// How the last turn ended. A turn cut short by a token or step limit
    /// otherwise looks exactly like one that finished, and a refusal changes
    /// what the agent will see next. Cleared when the next prompt starts.
    #[serde(default)]
    pub stop_reason: Option<String>,
}
impl AgentSnapshot {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            status: "connecting".into(),
            agent_name: "Local agent".into(),
            executable: String::new(),
            models: vec![],
            model_config_id: None,
            model_id: None,
            modes: vec![],
            mode_config_id: None,
            mode_id: None,
            session_id: None,
            messages: vec![],
            reviews: vec![],
            auth_methods: vec![],
            error: None,
            activity: "connecting".into(),
            last_activity_at: chrono::Utc::now().timestamp_millis(),
            turn_started_at: None,
            resumed: false,
            stop_reason: None,
        }
    }
    pub fn push(&mut self, role: &str, text: String) {
        self.messages.push(AgentMessage {
            id: uuid::Uuid::new_v4().to_string(),
            role: role.into(),
            text: bounded(&text, 48 * 1024),
            status: None,
            detail: None,
        });
        self.trim();
    }
    pub fn trim(&mut self) {
        while self.messages.len() > 200
            || (self.messages.len() > 1
                && self
                    .messages
                    .iter()
                    .map(|m| m.text.len() + m.detail.as_ref().map_or(0, String::len))
                    .sum::<usize>()
                    > 512 * 1024)
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub label: String,
    pub agent_name: String,
    pub active: bool,
    pub available: bool,
}

/// What an agent asks the project's window to show: a diagram (optionally one of its nodes),
/// a doc, or a data source. The window ignores requests for a project it is not showing.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Navigation {
    pub project_id: String,
    /// `diagram`, `doc` or `source`.
    pub kind: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
}
