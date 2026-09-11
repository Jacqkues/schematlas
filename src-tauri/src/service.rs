use crate::{
    connectors,
    domain::*,
    error::{AppError, Result},
    openapi,
    repository::ProjectRepository,
};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
};
use tokio::sync::Mutex;
pub struct Workspace {
    pub repository: ProjectRepository,
    pub(crate) folders_root: PathBuf,
    pub(crate) folders_lock: Mutex<()>,
    sessions: Mutex<HashMap<String, ConnectionRequest>>,
    api_sessions: Mutex<HashMap<String, crate::execution::http::ApiConnection>>,
}
impl Workspace {
    pub async fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            repository: ProjectRepository::open(path).await?,
            folders_root: std::fs::canonicalize(
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new(".")),
            )?
            .join("projects"),
            folders_lock: Mutex::new(()),
            sessions: Mutex::new(HashMap::new()),
            api_sessions: Mutex::new(HashMap::new()),
        })
    }
    pub async fn connect(
        &self,
        project_id: &str,
        name: &str,
        request: ConnectionRequest,
        source_id: Option<String>,
    ) -> Result<Project> {
        let name = valid_name(name)?;
        self.repository.get(project_id).await?;
        let graph = connectors::inspect(&request).await?;
        let id = source_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let source = Source {
            groups: vec![],
            layout_backup: None,
            api_base_url: None,
            id: id.clone(),
            name,
            kind: "database".into(),
            database_kind: Some(request.kind.clone()),
            imported_at: chrono::Utc::now().to_rfc3339(),
            graph,
            positions: BTreeMap::new(),
        };
        let project = self
            .repository
            .mutate(project_id, |p| {
                if source_id.is_some() {
                    let old = p.source_mut(&id)?;
                    let positions = old.positions.clone();
                    let groups = old.groups.clone();
                    *old = source;
                    old.positions = positions;
                    old.groups = groups;
                    let ids = old
                        .graph
                        .entities
                        .iter()
                        .map(|e| e.id.clone())
                        .collect::<std::collections::HashSet<_>>();
                    old.positions.retain(|id, _| ids.contains(id));
                    for group in &mut old.groups {
                        group.node_ids.retain(|id| ids.contains(id));
                    }
                    old.groups.retain(|g| !g.node_ids.is_empty());
                } else {
                    p.sources.push(source);
                }
                Ok(())
            })
            .await?;
        self.sessions.lock().await.insert(id, request);
        Ok(project)
    }
    pub async fn refresh(&self, project_id: &str, source_id: &str) -> Result<Project> {
        let p = self.repository.get(project_id).await?;
        let source = p
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .ok_or(AppError::NotFound)?;
        let request = self
            .sessions
            .lock()
            .await
            .get(source_id)
            .cloned()
            .ok_or(AppError::Reconnect)?;
        self.connect(project_id, &source.name, request, Some(source_id.into()))
            .await
    }
    pub async fn import(&self, project_id: &str, text: &str) -> Result<Project> {
        let (name, graph) = openapi::parse(text)?;
        let source = Source {
            groups: vec![],
            layout_backup: None,
            api_base_url: None,
            id: uuid::Uuid::new_v4().to_string(),
            name,
            kind: "openapi".into(),
            database_kind: None,
            imported_at: chrono::Utc::now().to_rfc3339(),
            graph,
            positions: BTreeMap::new(),
        };
        self.repository
            .mutate(project_id, |p| {
                p.sources.push(source);
                Ok(())
            })
            .await
    }
    pub async fn remove_source(&self, project_id: &str, source_id: &str) -> Result<Project> {
        let p = self
            .repository
            .mutate(project_id, |p| {
                p.source_mut(source_id)?;
                p.sources.retain(|s| s.id != source_id);
                Ok(())
            })
            .await?;
        self.sessions.lock().await.remove(source_id);
        self.api_sessions.lock().await.remove(source_id);
        Ok(p)
    }
    pub async fn delete_project(&self, id: &str) -> Result<()> {
        let p = self.repository.get(id).await?;
        self.repository.delete(id).await?;
        let mut sessions = self.sessions.lock().await;
        for s in p.sources {
            sessions.remove(&s.id);
            self.api_sessions.lock().await.remove(&s.id);
        }
        Ok(())
    }
    pub async fn save_positions(
        &self,
        project_id: &str,
        source_id: &str,
        positions: BTreeMap<String, Position>,
    ) -> Result<Project> {
        self.repository
            .mutate(project_id, |p| {
                crate::canvas::move_nodes(
                    p.source_mut(source_id)?,
                    positions
                        .into_iter()
                        .map(|(node_id, position)| crate::canvas::NodeMove {
                            node_id,
                            x: position.x,
                            y: position.y,
                        })
                        .collect(),
                )
            })
            .await
    }
    pub async fn connection(&self, project_id: &str, source_id: &str) -> Result<ConnectionRequest> {
        let project = self.repository.get(project_id).await?;
        if !project
            .sources
            .iter()
            .any(|s| s.id == source_id && s.kind == "database")
        {
            return Err(AppError::NotFound);
        }
        self.sessions
            .lock()
            .await
            .get(source_id)
            .cloned()
            .ok_or(AppError::Reconnect)
    }
    pub async fn configure_api(
        &self,
        project_id: &str,
        source_id: &str,
        config: crate::execution::http::ApiConnection,
    ) -> Result<Project> {
        let url = crate::execution::http::validate_config(&config)?.to_string();
        let project = self
            .repository
            .mutate(project_id, |p| {
                let source = p.source_mut(source_id)?;
                if source.kind != "openapi" {
                    return Err(AppError::Validation("Select an OpenAPI source.".into()));
                }
                source.api_base_url = Some(url);
                Ok(())
            })
            .await?;
        self.api_sessions
            .lock()
            .await
            .insert(source_id.into(), config);
        Ok(project)
    }
    pub async fn api_connection(
        &self,
        project_id: &str,
        source_id: &str,
    ) -> Result<(Source, crate::execution::http::ApiConnection)> {
        let project = self.repository.get(project_id).await?;
        let source = project
            .sources
            .into_iter()
            .find(|s| s.id == source_id && s.kind == "openapi")
            .ok_or(AppError::NotFound)?;
        let config = self.api_sessions.lock().await.get(source_id).cloned()
            .ok_or_else(|| AppError::Validation("Configure this API connection in Schematlas before sending requests. Authentication headers are session-only.".into()))?;
        Ok((source, config))
    }
    pub async fn demo(&self) -> Result<Project> {
        let mut p = Project::new(
            "Northstar Commerce".into(),
            "An example workspace. Explore a real SQLite schema and its companion API.".into(),
        )?;
        let graph: Graph =
            serde_json::from_str(include_str!("../../examples/commerce-schema.json"))?;
        p.sources.push(Source {
            groups: vec![],
            layout_backup: None,
            api_base_url: None,
            id: uuid::Uuid::new_v4().to_string(),
            name: "Commerce database".into(),
            kind: "database".into(),
            database_kind: Some(DatabaseKind::Sqlite),
            imported_at: chrono::Utc::now().to_rfc3339(),
            graph,
            positions: BTreeMap::new(),
        });
        let (name, graph) = openapi::parse(include_str!("../../examples/commerce.openapi.json"))?;
        p.sources.push(Source {
            groups: vec![],
            layout_backup: None,
            api_base_url: None,
            id: uuid::Uuid::new_v4().to_string(),
            name,
            kind: "openapi".into(),
            database_kind: None,
            imported_at: chrono::Utc::now().to_rfc3339(),
            graph,
            positions: BTreeMap::new(),
        });
        let layout: CanvasLayout =
            serde_json::from_str(include_str!("../../examples/commerce-layout.json"))?;
        p.sources[0].groups = layout.groups;
        p.sources[0].positions = layout.positions;
        self.repository.create(p).await
    }
}
