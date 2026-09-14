pub mod discovery;
pub mod mcp;
pub mod schema_tools;
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
    /// Apply a validated canvas edit, persist it, and push the new project to the open window.
    async fn save_canvas(
        &self,
        project_id: &str,
        edit: impl FnOnce(&mut crate::domain::Project) -> Result<()>,
    ) -> Result<crate::domain::Project> {
        let updated = self.workspace.repository.mutate(project_id, edit).await?;
        let _ = self.changes.send(updated.clone());
        Ok(updated)
    }
    /// Review one prepared statement, then run it and render the result as compact text.
    async fn run_sql(
        &self,
        session: &Session,
        project_id: &str,
        source: &crate::domain::Source,
        title: String,
        prepared: sql::Prepared,
        limit: usize,
    ) -> Result<Value> {
        let connection = self.workspace.connection(project_id, &source.id).await?;
        sql::validate(&connection.kind, &prepared.sql)?;
        let mut details =
            json!({"source":source.name,"databaseKind":connection.kind,"sql":prepared.sql});
        if let Some(note) = &prepared.note {
            details["note"] = json!(note);
        }
        self.approve(session, title, "sql", details).await?;
        // The user may have disconnected the source while the review was open.
        let connection = self.workspace.connection(project_id, &source.id).await?;
        let output = session
            .run_tool(sql::execute_with(&connection, &prepared.sql, limit))
            .await?;
        let mut text = output.render();
        if let Some(note) = prepared.note {
            text.push_str("\nnote: ");
            text.push_str(&note);
        }
        Ok(Value::String(text))
    }
    /// Tool results are plain text: the model reads them directly, so JSON envelopes only cost tokens.
    pub async fn tool(&self, token: &str, name: &str, args: Value) -> Result<Value> {
        let session = self.authorized(token).await?;
        let project = self.workspace.repository.get(&session.project_id).await?;
        match name {
            "list_sources" => Ok(Value::String(render_sources(&project))),
            "search_schema" => {
                let args: mcp::SearchArgs = serde_json::from_value(args)?;
                Ok(Value::String(schema_tools::search(
                    &project,
                    &args.query,
                    args.limit.unwrap_or(0),
                )?))
            }
            "get_schema" => {
                let args: mcp::SchemaArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                Ok(Value::String(schema_tools::render_schema(
                    source,
                    args.namespace.as_deref(),
                    args.offset.unwrap_or(0),
                    args.limit.unwrap_or(0),
                )))
            }
            "describe_table" => {
                let args: mcp::TableArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let entity = schema_tools::resolve_entity(source, &args.table)?;
                Ok(Value::String(schema_tools::describe(source, entity)))
            }
            "find_join_path" => {
                let args: mcp::JoinArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let from = schema_tools::resolve_entity(source, &args.from)?;
                let to = schema_tools::resolve_entity(source, &args.to)?;
                Ok(Value::String(schema_tools::join_path(source, from, to)))
            }
            "get_canvas" => {
                let args: mcp::SourceArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                Ok(Value::String(schema_tools::render_canvas(source)))
            }
            "move_nodes" => {
                let args: mcp::MoveNodesArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let source_id = source.id.clone();
                let mut nodes = Vec::with_capacity(args.nodes.len());
                for node in &args.nodes {
                    nodes.push(crate::canvas::NodeMove {
                        node_id: schema_tools::resolve_entity(source, &node.table)?
                            .id
                            .clone(),
                        x: node.x,
                        y: node.y,
                    });
                }
                let moved = nodes.len();
                self.save_canvas(&project.id, move |p| {
                    crate::canvas::move_nodes(p.source_mut(&source_id)?, nodes)
                })
                .await?;
                Ok(Value::String(format!(
                    "Saved. {moved} node{} moved.",
                    if moved == 1 { "" } else { "s" }
                )))
            }
            "create_group" => {
                let args: mcp::GroupArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let source_id = source.id.clone();
                let mut node_ids = Vec::with_capacity(args.tables.len());
                for table in &args.tables {
                    node_ids.push(schema_tools::resolve_entity(source, table)?.id.clone());
                }
                let members = node_ids.len();
                let requested = args.group_id.clone();
                let group = crate::canvas::GroupArgs {
                    source_id: source_id.clone(),
                    name: args.name,
                    node_ids,
                    group_id: args.group_id,
                    color: args.color,
                };
                let name = group.name.clone();
                let updated = self
                    .save_canvas(&project.id, move |p| {
                        crate::canvas::set_group(p.source_mut(&source_id)?, group).map(|_| ())
                    })
                    .await?;
                // A new group is appended, so the last one is the group just created.
                let id = requested.or_else(|| {
                    updated
                        .sources
                        .iter()
                        .find(|s| s.id == source.id)
                        .and_then(|s| s.groups.last())
                        .map(|g| g.id.clone())
                });
                Ok(Value::String(format!(
                    "Saved group \"{name}\" ({}) with {members} member{}.",
                    id.unwrap_or_else(|| "unknown id".into()),
                    if members == 1 { "" } else { "s" }
                )))
            }
            "remove_group" => {
                let args: mcp::RemoveGroupArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let source_id = source.id.clone();
                let group_id = args.group_id.clone();
                self.save_canvas(&project.id, move |p| {
                    crate::canvas::remove_group(p.source_mut(&source_id)?, &group_id)
                })
                .await?;
                Ok(Value::String(format!("Removed group {}.", args.group_id)))
            }
            "table_stats" | "sample_rows" => {
                let sampling = name == "sample_rows";
                let (source_ref, table_ref, limit) = if sampling {
                    let args: mcp::SampleArgs = serde_json::from_value(args)?;
                    (args.source, args.table, sql::clamp_limit(args.limit))
                } else {
                    let args: mcp::TableArgs = serde_json::from_value(args)?;
                    (args.source, args.table, sql::clamp_limit(None))
                };
                let source = schema_tools::resolve_source(&project, &source_ref)?;
                let entity = schema_tools::resolve_entity(source, &table_ref)?;
                if !["table", "view"].contains(&entity.kind.as_str()) {
                    return Err(AppError::Validation(
                        "Choose a table or view from a connected database.".into(),
                    ));
                }
                let connection = self.workspace.connection(&project.id, &source.id).await?;
                let prepared = if sampling {
                    sql::sample_rows(&connection.kind, &entity.namespace, &entity.name, limit)?
                } else {
                    sql::table_stats(&connection.kind, &entity.namespace, &entity.name)?
                };
                let title = format!(
                    "{} · {}",
                    if sampling { "Read rows" } else { "Table stats" },
                    schema_tools::display_ref(entity)
                );
                self.run_sql(&session, &project.id, source, title, prepared, limit)
                    .await
            }
            "explain_sql" => {
                let args: mcp::ExplainArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let connection = self.workspace.connection(&project.id, &source.id).await?;
                let prepared = sql::explain(&connection.kind, &args.sql)?;
                let title = format!("Explain SQL · {}", source.name);
                self.run_sql(
                    &session,
                    &project.id,
                    source,
                    title,
                    prepared,
                    sql::MAX_ROWS,
                )
                .await
            }
            "query_sql" => {
                let args: mcp::SqlArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let limit = sql::clamp_limit(args.limit);
                let title = format!("Run SQL · {}", source.name);
                let prepared = sql::Prepared {
                    sql: args.sql,
                    note: None,
                };
                self.run_sql(&session, &project.id, source, title, prepared, limit)
                    .await
            }
            "request_http" => {
                let args: mcp::HttpArgs = serde_json::from_value(args)?;
                let source = schema_tools::resolve_source(&project, &args.source)?;
                let operation = schema_tools::resolve_entity(source, &args.operation)?;
                let request = http::HttpRequest {
                    source_id: source.id.clone(),
                    operation_id: operation.id.clone(),
                    path_parameters: args.path_parameters,
                    query: args.query,
                    body: args.body,
                };
                let (api_source, config) = self
                    .workspace
                    .api_connection(&project.id, &source.id)
                    .await?;
                let (method, url) = http::resolve(&api_source, &config, &request)?;
                self.approve(&session,format!("{} · {}",method,api_source.name),"http",json!({"method":method,"url":url.as_str(),"body":request.body,"authentication":"Configured headers are included; their values stay private."})).await?;
                self.workspace
                    .api_connection(&project.id, &source.id)
                    .await?;
                session
                    .run_tool(http::execute(&api_source, &config, &request))
                    .await
            }
            _ => Err(AppError::Validation("Unknown project tool.".into())),
        }
    }
}
/// One line per source, naming the reference the agent should use in later calls.
fn render_sources(project: &crate::domain::Project) -> String {
    use std::collections::BTreeSet;
    let mut text = format!("Project: {}\n", project.name);
    if project.sources.is_empty() {
        text.push_str("No sources yet. The user connects a database or imports an OpenAPI file in Schematlas.");
        return text;
    }
    for source in &project.sources {
        let namespaces: BTreeSet<&str> = source
            .graph
            .entities
            .iter()
            .map(|e| e.namespace.as_str())
            .collect();
        let database = source.kind == "database";
        text.push_str(if database { "db  " } else { "api " });
        text.push_str(&source.name);
        text.push_str(" — ");
        if let Some(kind) = &source.database_kind {
            text.push_str(
                serde_json::to_value(kind)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default()
                    .as_str(),
            );
            text.push_str(", ");
        }
        text.push_str(&format!(
            "{} entities, {} relationships",
            source.graph.entities.len(),
            source.graph.relations.len()
        ));
        if !namespaces.is_empty() {
            text.push_str(if database {
                "; schemas: "
            } else {
                "; groups: "
            });
            text.push_str(&namespaces.into_iter().collect::<Vec<_>>().join(", "));
        }
        if !database {
            match &source.api_base_url {
                Some(base) => text.push_str(&format!("; base: {base}")),
                None => text.push_str("; no API connection configured yet"),
            }
        }
        text.push('\n');
    }
    text.truncate(text.trim_end().len());
    text
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
