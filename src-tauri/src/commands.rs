//! Thin IPC boundary. Domain operations and I/O live outside Tauri commands.
use crate::agents::{
    types::{AgentConfig, AgentSnapshot},
    AgentHub,
};
use crate::{domain::*, error::Result, service::Workspace};
use std::{collections::BTreeMap, sync::Arc};
use tauri::State;
#[tauri::command]
pub async fn list_projects(state: State<'_, Arc<Workspace>>) -> Result<Vec<Project>> {
    state.repository.list().await
}
#[tauri::command]
pub async fn create_project(
    state: State<'_, Arc<Workspace>>,
    name: String,
    description: String,
) -> Result<Project> {
    state
        .repository
        .create(Project::new(name, description)?)
        .await
}
#[tauri::command]
pub async fn rename_project(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    name: String,
    description: String,
) -> Result<Project> {
    let valid = Project::new(name, description)?;
    state
        .repository
        .mutate(&project_id, |p| {
            p.name = valid.name;
            p.description = valid.description;
            Ok(())
        })
        .await
}
#[tauri::command]
pub async fn delete_project(
    hub: State<'_, Arc<AgentHub>>,
    state: State<'_, Arc<Workspace>>,
    project_id: String,
) -> Result<()> {
    hub.disconnect(&project_id).await;
    state.delete_project(&project_id).await
}
#[tauri::command]
pub async fn connect_database(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    name: String,
    request: ConnectionRequest,
    source_id: Option<String>,
) -> Result<Project> {
    state.connect(&project_id, &name, request, source_id).await
}
#[tauri::command]
pub async fn refresh_source(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
) -> Result<Project> {
    state.refresh(&project_id, &source_id).await
}
#[tauri::command]
pub async fn import_openapi(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    document: String,
) -> Result<Project> {
    state.import(&project_id, &document).await
}
#[tauri::command]
pub async fn remove_source(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
) -> Result<Project> {
    state.remove_source(&project_id, &source_id).await
}
#[tauri::command]
pub async fn save_positions(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
    positions: BTreeMap<String, Position>,
) -> Result<Project> {
    state
        .save_positions(&project_id, &source_id, positions)
        .await
}
#[tauri::command]
pub async fn create_demo(state: State<'_, Arc<Workspace>>) -> Result<Project> {
    state.demo().await
}
#[tauri::command]
pub async fn export_source(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
    path: String,
) -> Result<()> {
    let p = state.repository.get(&project_id).await?;
    let source = p
        .sources
        .iter()
        .find(|s| s.id == source_id)
        .ok_or(crate::error::AppError::NotFound)?;
    if std::path::Path::new(&path)
        .extension()
        .and_then(|s| s.to_str())
        != Some("json")
    {
        return Err(crate::error::AppError::Validation(
            "Choose a .json destination.".into(),
        ));
    }
    tokio::fs::write(path, serde_json::to_vec_pretty(source)?).await?;
    Ok(())
}

#[tauri::command]
pub async fn configure_api(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
    config: crate::execution::http::ApiConnection,
) -> Result<Project> {
    state.configure_api(&project_id, &source_id, config).await
}
#[tauri::command]
pub async fn agent_connect(
    hub: State<'_, Arc<AgentHub>>,
    project_id: String,
    config: AgentConfig,
) -> Result<()> {
    hub.connect(project_id, config).await
}
#[tauri::command]
pub async fn agent_status(
    hub: State<'_, Arc<AgentHub>>,
    project_id: String,
) -> Result<Option<AgentSnapshot>> {
    Ok(match hub.get(&project_id).await {
        Ok(s) => Some(s.snapshot.lock().await.clone()),
        Err(_) => None,
    })
}
#[tauri::command]
pub async fn agent_prompt(
    hub: State<'_, Arc<AgentHub>>,
    project_id: String,
    text: String,
) -> Result<()> {
    hub.get(&project_id).await?.prompt(text).await
}
#[tauri::command]
pub async fn agent_cancel(hub: State<'_, Arc<AgentHub>>, project_id: String) -> Result<()> {
    hub.get(&project_id).await?.cancel().await
}
#[tauri::command]
pub async fn agent_disconnect(hub: State<'_, Arc<AgentHub>>, project_id: String) -> Result<()> {
    hub.disconnect(&project_id).await;
    Ok(())
}
#[tauri::command]
pub async fn agent_decide(
    hub: State<'_, Arc<AgentHub>>,
    project_id: String,
    review_id: String,
    option: Option<String>,
) -> Result<()> {
    hub.get(&project_id).await?.decide(&review_id, option).await
}
#[tauri::command]
pub async fn agent_authenticate(
    hub: State<'_, Arc<AgentHub>>,
    project_id: String,
    method_id: String,
) -> Result<()> {
    hub.authenticate(&project_id, method_id).await
}

#[tauri::command]
pub async fn discover_agents() -> Result<Vec<crate::agents::discovery::InstalledAgent>> {
    tokio::task::spawn_blocking(crate::agents::discovery::discover)
        .await
        .map_err(|_| crate::error::AppError::Agent("Agent detection failed.".into()))
}
#[tauri::command]
pub async fn undo_canvas(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
) -> Result<Project> {
    state
        .repository
        .mutate(&project_id, |p| {
            crate::canvas::undo(p.source_mut(&source_id)?)
        })
        .await
}
#[tauri::command]
pub async fn remove_canvas_group(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    source_id: String,
    group_id: String,
) -> Result<Project> {
    state
        .repository
        .mutate(&project_id, |p| {
            crate::canvas::remove_group(p.source_mut(&source_id)?, &group_id)
        })
        .await
}

#[tauri::command]
pub async fn set_canvas_group(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
    args: crate::canvas::GroupArgs,
) -> Result<Project> {
    state
        .repository
        .mutate(&project_id, |p| {
            let source_id = args.source_id.clone();
            crate::canvas::set_group(p.source_mut(&source_id)?, args)?;
            Ok(())
        })
        .await
}

#[tauri::command]
pub async fn agent_working_directory(
    state: State<'_, Arc<Workspace>>,
    project_id: String,
) -> Result<String> {
    state.working_directory(&project_id).await
}
