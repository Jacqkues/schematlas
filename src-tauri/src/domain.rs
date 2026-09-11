use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseKind {
    Postgres,
    Mysql,
    Mariadb,
    Sqlite,
    Mssql,
}

/// Credentials deliberately have no Debug or Serialize implementation.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRequest {
    pub kind: DatabaseKind,
    pub connection_string: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub primary_key: bool,
    pub required: bool,
    pub default_value: Option<String>,
    pub description: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub namespace: String,
    pub kind: String,
    pub description: Option<String>,
    pub fields: Vec<Field>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    /// None means this saved catalog predates unique-key inspection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_keys: Option<Vec<Vec<String>>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    pub id: String,
    pub source: String,
    pub target: String,
    pub source_field: Option<String>,
    pub target_field: Option<String>,
    pub label: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Graph {
    pub entities: Vec<Entity>,
    pub relations: Vec<Relation>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasGroup {
    pub id: String,
    pub name: String,
    pub node_ids: Vec<String>,
    #[serde(default = "default_group_color")]
    pub color: String,
}
pub fn default_group_color() -> String {
    "#879b91".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CanvasLayout {
    pub positions: BTreeMap<String, Position>,
    pub groups: Vec<CanvasGroup>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default)]
    pub groups: Vec<CanvasGroup>,
    #[serde(default)]
    pub layout_backup: Option<CanvasLayout>,
    #[serde(default)]
    pub api_base_url: Option<String>,
    pub id: String,
    pub name: String,
    pub kind: String,
    pub database_kind: Option<DatabaseKind>,
    pub imported_at: String,
    pub graph: Graph,
    pub positions: BTreeMap<String, Position>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[serde(default)]
    pub working_directory: Option<String>,
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
    pub sources: Vec<Source>,
}
impl Project {
    pub fn new(name: String, description: String) -> Result<Self> {
        let name = valid_name(&name)?;
        if description.len() > 2000 {
            return Err(AppError::Validation(
                "Description exceeds 2,000 characters.".into(),
            ));
        }
        let now = chrono::Utc::now().to_rfc3339();
        Ok(Self {
            working_directory: None,
            id: uuid::Uuid::new_v4().to_string(),
            name,
            description,
            created_at: now.clone(),
            updated_at: now,
            sources: vec![],
        })
    }
    pub fn source_mut(&mut self, id: &str) -> Result<&mut Source> {
        self.sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(AppError::NotFound)
    }
}
pub fn valid_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation(
            "Use a name between 1 and 80 characters.".into(),
        ));
    }
    Ok(name.to_owned())
}
/// JSON tuples avoid collisions when identifiers contain dots or punctuation.
pub fn entity_id(namespace: &str, name: &str) -> String {
    serde_json::json!([namespace, name]).to_string()
}
impl Graph {
    pub fn validate(&self) -> Result<()> {
        use std::collections::HashSet;
        if self.entities.len() > 5000 || self.relations.len() > 20000 {
            return Err(AppError::Validation(
                "This graph exceeds the 5,000 node / 20,000 relationship limit.".into(),
            ));
        }
        let ids: HashSet<_> = self.entities.iter().map(|e| &e.id).collect();
        if ids.len() != self.entities.len() {
            return Err(AppError::Validation("Duplicate entity identifiers.".into()));
        }
        if self
            .relations
            .iter()
            .any(|r| !ids.contains(&r.source) || !ids.contains(&r.target))
        {
            return Err(AppError::Validation(
                "A relationship refers to a missing entity.".into(),
            ));
        }
        Ok(())
    }
}
