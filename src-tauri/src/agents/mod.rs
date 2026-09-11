pub mod discovery;
pub mod mcp;
pub mod session;
pub mod types;
use crate::{
    error::{AppError, Result},
    execution::{http, sql},
    service::Workspace,
};
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use session::Session;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, Mutex};
use types::{AgentConfig, AgentSnapshot, ReviewOption};

pub struct AgentHub {
    pub workspace: Arc<Workspace>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
    pub events: broadcast::Sender<AgentSnapshot>,
    pub changes: broadcast::Sender<crate::domain::Project>,
    endpoint: String,
}
impl AgentHub {
    pub async fn start(workspace: Arc<Workspace>) -> Result<Arc<Self>> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/tool", listener.local_addr()?);
        let (events, _) = broadcast::channel(128);
        let (changes, _) = broadcast::channel(32);
        let hub = Arc::new(Self {
            workspace,
            changes,
            sessions: Mutex::new(HashMap::new()),
            events,
            endpoint,
        });
        let router = Router::new()
            .route("/tool", post(dispatch))
            .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
            .with_state(hub.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(hub)
    }
    pub async fn get(&self, id: &str) -> Result<Arc<Session>> {
        self.sessions
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::Agent("Connect a local agent first.".into()))
    }
    pub async fn connect(&self, id: String, mut config: AgentConfig) -> Result<()> {
        config.cwd = self
            .workspace
            .remember_working_directory(&id, &config.cwd)
            .await?;
        // Serialize replacement so concurrent connect commands cannot orphan a process.
        let mut sessions = self.sessions.lock().await;
        if let Some(previous) = sessions.remove(&id) {
            previous.stop().await;
        }
        let session = Session::spawn(id.clone(), config, self.events.clone()).await?;
        sessions.insert(id, session.clone());
        drop(sessions);
        if let Err(error) = session.initialize().await {
            session.stop().await;
            session.set_error(&error.to_string()).await;
            return Err(error);
        }
        session
            .new_session(&self.endpoint, &std::env::current_exe()?.to_string_lossy())
            .await
    }
    pub async fn authenticate(&self, id: &str, method: String) -> Result<()> {
        let session = self.get(id).await?;
        if !session
            .snapshot
            .lock()
            .await
            .auth_methods
            .iter()
            .any(|m| m["id"].as_str() == Some(&method))
        {
            return Err(AppError::Validation(
                "Unknown authentication method.".into(),
            ));
        }
        session
            .rpc(
                "authenticate",
                json!({"methodId":method}),
                Duration::from_secs(180),
            )
            .await?;
        session
            .new_session(&self.endpoint, &std::env::current_exe()?.to_string_lossy())
            .await
    }
    pub async fn disconnect(&self, id: &str) {
        if let Some(session) = self.sessions.lock().await.remove(id) {
            session.stop().await;
        }
    }
    pub async fn shutdown(&self) {
        for (_, session) in self.sessions.lock().await.drain() {
            session.stop().await;
        }
    }
    async fn authorized(&self, token: &str) -> Result<Arc<Session>> {
        let session = self
            .sessions
            .lock()
            .await
            .values()
            .find(|s| s.token == token)
            .cloned()
            .ok_or(AppError::Declined)?;
        if !["ready", "running"].contains(&session.snapshot.lock().await.status.as_str()) {
            return Err(AppError::Declined);
        }
        Ok(session)
    }
    async fn approve(
        &self,
        session: &Session,
        title: String,
        kind: &str,
        details: Value,
    ) -> Result<()> {
        let options = vec![
            ReviewOption {
                option_id: "allow".into(),
                name: "Allow once".into(),
                kind: "allow_once".into(),
            },
            ReviewOption {
                option_id: "reject".into(),
                name: "Reject".into(),
                kind: "reject_once".into(),
            },
        ];
        if session
            .review(title, kind.into(), details, options)
            .await?
            .as_deref()
            != Some("allow")
        {
            return Err(AppError::Declined);
        }
        self.authorized(&session.token).await?;
        Ok(())
    }
    pub async fn tool(&self, token: &str, name: &str, args: Value) -> Result<Value> {
        let session = self.authorized(token).await?;
        let project = self.workspace.repository.get(&session.project_id).await?;
        match name {
            "list_sources" => Ok(
                json!({"project":project.name,"sources":project.sources.iter().map(|s|json!({"id":s.id,"name":s.name,"kind":s.kind,"databaseKind":s.database_kind,"apiBaseUrl":s.api_base_url,"entities":s.graph.entities.len(),"namespaces":s.graph.entities.iter().map(|e|&e.namespace).collect::<std::collections::BTreeSet<_>>()})).collect::<Vec<_>>()}),
            ),
            "get_schema" => {
                let args: mcp::SchemaArgs = serde_json::from_value(args)?;
                let source = project
                    .sources
                    .into_iter()
                    .find(|s| s.id == args.source_id)
                    .ok_or(AppError::NotFound)?;
                let mut graph = source.graph;
                if let Some(namespace) = args.namespace {
                    graph.entities.retain(|e| e.namespace == namespace);
                    let ids = graph
                        .entities
                        .iter()
                        .map(|e| e.id.as_str())
                        .collect::<std::collections::HashSet<_>>();
                    graph.relations.retain(|r| {
                        ids.contains(r.source.as_str()) || ids.contains(r.target.as_str())
                    });
                }
                Ok(serde_json::to_value(graph)?)
            }
            "get_canvas" => {
                let args: crate::canvas::SourceArgs = serde_json::from_value(args)?;
                let source = project
                    .sources
                    .iter()
                    .find(|s| s.id == args.source_id)
                    .ok_or(AppError::NotFound)?;
                Ok(
                    json!({"sourceId":source.id,"positions":source.positions,"groups":source.groups,"nodeWidth":284,"nodes":source.graph.entities.iter().map(|e|json!({"id":e.id,"name":e.name,"namespace":e.namespace,"estimatedHeight":80+29*e.fields.len().min(9)+if e.fields.len()>9{29}else{0}})).collect::<Vec<_>>(),"note":"Unpositioned nodes use the client auto-layout. move_nodes sets absolute canvas coordinates. Group overlays follow member bounds."}),
                )
            }
            "move_nodes" | "create_group" | "remove_group" => {
                let updated = self
                    .workspace
                    .repository
                    .mutate(&project.id, |p| {
                        match name {
                            "move_nodes" => {
                                let args: crate::canvas::MoveNodes = serde_json::from_value(args)?;
                                crate::canvas::move_nodes(
                                    p.source_mut(&args.source_id)?,
                                    args.nodes,
                                )?;
                            }
                            "create_group" => {
                                let args: crate::canvas::GroupArgs = serde_json::from_value(args)?;
                                let source_id = args.source_id.clone();
                                crate::canvas::set_group(p.source_mut(&source_id)?, args)?;
                            }
                            _ => {
                                let args: crate::canvas::RemoveGroup =
                                    serde_json::from_value(args)?;
                                crate::canvas::remove_group(
                                    p.source_mut(&args.source_id)?,
                                    &args.group_id,
                                )?;
                            }
                        }
                        Ok(())
                    })
                    .await?;
                let _ = self.changes.send(updated.clone());
                Ok(
                    json!({"saved":true,"sources":updated.sources.iter().map(|s|json!({"id":s.id,"positions":s.positions,"groups":s.groups})).collect::<Vec<_>>()}),
                )
            }
            "query_sql" => {
                let args: mcp::SqlArgs = serde_json::from_value(args)?;
                let connection = self
                    .workspace
                    .connection(&project.id, &args.source_id)
                    .await?;
                sql::validate(&connection.kind, &args.sql)?;
                let source = project
                    .sources
                    .iter()
                    .find(|s| s.id == args.source_id)
                    .ok_or(AppError::NotFound)?;
                self.approve(
                    &session,
                    format!("Run SQL · {}", source.name),
                    "sql",
                    json!({"source":source.name,"databaseKind":connection.kind,"sql":args.sql}),
                )
                .await?;
                self.workspace
                    .connection(&project.id, &args.source_id)
                    .await?;
                Ok(serde_json::to_value(
                    session
                        .run_tool(sql::execute(&connection, &args.sql))
                        .await?,
                )?)
            }
            "request_http" => {
                let args: http::HttpRequest = serde_json::from_value(args)?;
                let (source, config) = self
                    .workspace
                    .api_connection(&project.id, &args.source_id)
                    .await?;
                let (method, url) = http::resolve(&source, &config, &args)?;
                self.approve(&session,format!("{} · {}",method,source.name),"http",json!({"method":method,"url":url.as_str(),"body":args.body,"authentication":"Configured headers are included; their values stay private."})).await?;
                self.workspace
                    .api_connection(&project.id, &args.source_id)
                    .await?;
                session
                    .run_tool(http::execute(&source, &config, &args))
                    .await
            }
            _ => Err(AppError::Validation("Unknown project tool.".into())),
        }
    }
}
#[derive(Deserialize)]
struct ToolRequest {
    name: String,
    #[serde(default)]
    arguments: Value,
}
async fn dispatch(
    State(hub): State<Arc<AgentHub>>,
    headers: HeaderMap,
    Json(request): Json<ToolRequest>,
) -> std::result::Result<Json<Value>, StatusCode> {
    if headers.contains_key("origin") {
        return Err(StatusCode::FORBIDDEN);
    }
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    hub.authorized(token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(Json(
        match hub.tool(token, &request.name, request.arguments).await {
            Ok(value) => json!({"result":value}),
            Err(error) => json!({"error":error.to_string()}),
        },
    ))
}

#[cfg(test)]
mod tests;
