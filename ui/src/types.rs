//! Data contracts shared with the Rust backend (serde camelCase, same shapes as `src-tauri/src/domain.rs`).
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    #[serde(default)]
    pub data_type: String,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub primary_key: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_value: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub namespace: String,
    /// `table`, `view`, `schema`, or `operation`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_keys: Option<Vec<Vec<String>>>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub source_field: Option<String>,
    #[serde(default)]
    pub target_field: Option<String>,
    #[serde(default)]
    pub label: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutBackup {
    #[serde(default)]
    pub positions: HashMap<String, Position>,
    #[serde(default)]
    pub groups: Vec<CanvasGroup>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    /// `database` or `openapi`.
    pub kind: String,
    #[serde(default)]
    pub database_kind: Option<String>,
    #[serde(default)]
    pub imported_at: String,
    #[serde(default)]
    pub graph: Graph,
    #[serde(default)]
    pub positions: HashMap<String, Position>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<CanvasGroup>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_backup: Option<LayoutBackup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
}

impl Source {
    pub fn is_database(&self) -> bool {
        self.kind == "database"
    }
    pub fn groups(&self) -> &[CanvasGroup] {
        self.groups.as_deref().unwrap_or(&[])
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[serde(default)]
    pub explorer: crate::explorer::Explorer,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub sources: Vec<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sync: Option<serde_json::Value>,
}

pub const DATABASE_KINDS: [(&str, &str); 5] = [
    ("postgres", "PostgreSQL"),
    ("mysql", "MySQL"),
    ("mariadb", "MariaDB"),
    ("sqlite", "SQLite"),
    ("mssql", "SQL Server"),
];

pub fn database_name(kind: &str) -> &'static str {
    DATABASE_KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, label)| *label)
        .unwrap_or("Database")
}

// Agent session snapshots emitted by the backend.

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessage {
    pub id: String,
    pub role: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub status: Option<String>,
    /// What a tool call returned. `text` stays its one-line title.
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOption {
    pub option_id: String,
    pub name: String,
    #[serde(default)]
    pub kind: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub details: serde_json::Value,
    #[serde(default)]
    pub options: Vec<ReviewOption>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentSnapshot {
    pub project_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub agent_name: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub messages: Vec<AgentMessage>,
    #[serde(default)]
    pub reviews: Vec<Review>,
    #[serde(default)]
    pub auth_methods: Vec<AuthMethod>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub activity: String,
    #[serde(default)]
    pub last_activity_at: f64,
    #[serde(default)]
    pub turn_started_at: Option<f64>,
    /// These messages were replayed by the agent on reconnect rather than
    /// produced here.
    #[serde(default)]
    pub resumed: bool,
    /// Set when the last turn ended for any reason other than finishing.
    #[serde(default)]
    pub stop_reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAgent {
    pub name: String,
    pub executable: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub acp_ready: bool,
    #[serde(default)]
    pub note: String,
}

// Repository atlas: `.schematlas/` diagrams and docs. Diagram fields use the file's snake_case.

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CodeRef {
    Text(String),
    Full {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_line: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        symbol: Option<String>,
    },
}
impl CodeRef {
    /// `path:line-end`, the form the backend resolves.
    pub fn reference(&self) -> String {
        match self {
            CodeRef::Text(text) => text.clone(),
            CodeRef::Full {
                path,
                line,
                end_line,
                ..
            } => {
                let mut text = path.clone();
                if let Some(line) = line {
                    text.push_str(&format!(":{line}"));
                    if let Some(end) = end_line.filter(|e| e != line) {
                        text.push_str(&format!("-{end}"));
                    }
                }
                text
            }
        }
    }
    pub fn label(&self) -> String {
        match self {
            CodeRef::Full {
                symbol: Some(symbol),
                ..
            } => format!("{symbol} · {}", self.reference()),
            _ => self.reference(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NodeStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagramNode {
    pub id: String,
    #[serde(default = "default_node_kind")]
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_md: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_refs: Vec<CodeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<NodeStyle>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
fn default_node_kind() -> String {
    "module".into()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagramEdge {
    #[serde(default)]
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_md: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_refs: Vec<CodeRef>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiagramGroup {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default = "default_group_color")]
    pub color: String,
}
fn default_group_color() -> String {
    crate::layout::DEFAULT_GROUP_COLOR.into()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Diagram {
    #[serde(default = "diagram_version")]
    pub version: u32,
    #[serde(default = "default_diagram_kind")]
    pub kind: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_md: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    #[serde(default)]
    pub nodes: Vec<DiagramNode>,
    #[serde(default)]
    pub edges: Vec<DiagramEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<DiagramGroup>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub layout: std::collections::BTreeMap<String, Frame>,
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}
fn diagram_version() -> u32 {
    1
}
fn default_diagram_kind() -> String {
    "architecture".into()
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramFile {
    pub slug: String,
    pub hash: String,
    pub diagram: Diagram,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocFile {
    pub slug: String,
    pub hash: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub markdown: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoEntry {
    pub slug: String,
    #[serde(default)]
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub nodes: usize,
    #[serde(default)]
    pub edges: usize,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoListing {
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub is_git: bool,
    #[serde(default)]
    pub diagrams: Vec<RepoEntry>,
    #[serde(default)]
    pub docs: Vec<RepoEntry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub path: String,
    pub start: u32,
    #[serde(default)]
    pub focus: Option<(u32, u32)>,
    #[serde(default)]
    pub lines: Vec<String>,
    #[serde(default)]
    pub total: u32,
}

// Team channel snapshots emitted by the backend's git sync.

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Author {
    pub key: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SyncEvent {
    pub id: String,
    #[serde(default)]
    pub author: Author,
    #[serde(default)]
    pub lamport: u64,
    #[serde(default)]
    pub ts: String,
    pub kind: String,
    #[serde(default)]
    pub body: serde_json::Value,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Presence {
    pub author: Author,
    #[serde(default)]
    pub ts: String,
    #[serde(default)]
    pub diagram: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSnapshot {
    pub project_id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub me: Option<Author>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub last_sync: Option<String>,
    #[serde(default)]
    pub events: Vec<SyncEvent>,
    #[serde(default)]
    pub presence: Vec<Presence>,
    #[serde(default)]
    pub pending: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    #[serde(default)]
    pub remotes: Vec<String>,
    #[serde(default)]
    pub remote: Option<String>,
    #[serde(default)]
    pub remote_url: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub has_email: bool,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub branch_exists: bool,
    #[serde(default)]
    pub error: Option<String>,
}
