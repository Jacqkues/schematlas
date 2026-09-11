pub mod agents;
pub mod canvas;
mod commands;
pub mod connectors;
pub mod domain;
pub mod error;
pub mod execution;
pub mod openapi;
mod project_folders;
pub mod repository;
pub mod service;
use std::sync::Arc;
use tauri::{Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let workspace = tauri::async_runtime::block_on(service::Workspace::open(
                &data.join("workspace.sqlite"),
            ))?;
            let workspace = Arc::new(workspace);
            let hub = tauri::async_runtime::block_on(agents::AgentHub::start(workspace.clone()))?;
            let mut changes = hub.changes.subscribe();
            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match changes.recv().await {
                        Ok(project) => {
                            let _ = update_handle.emit("workspace:update", project);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            let mut events = hub.events.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut pending = std::collections::HashMap::new();
                let mut frame = tokio::time::interval(std::time::Duration::from_millis(100));
                frame.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        event = events.recv() => match event {
                            Ok(snapshot) => { pending.insert(snapshot.project_id.clone(), snapshot); }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => { for (_, snapshot) in pending.drain() { let _ = handle.emit("agent:update", snapshot); } break; }
                        },
                        _ = frame.tick() => {
                            for (_, snapshot) in pending.drain() { let _ = handle.emit("agent:update", snapshot); }
                        }
                    }
                }
            });
            app.manage(hub);
            app.manage(workspace);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::create_project,
            commands::rename_project,
            commands::delete_project,
            commands::connect_database,
            commands::refresh_source,
            commands::import_openapi,
            commands::remove_source,
            commands::save_positions,
            commands::create_demo,
            commands::configure_api,
            commands::discover_agents,
            commands::undo_canvas,
            commands::remove_canvas_group,
            commands::set_canvas_group,
            commands::agent_connect,
            commands::agent_working_directory,
            commands::agent_status,
            commands::agent_prompt,
            commands::agent_cancel,
            commands::agent_disconnect,
            commands::agent_decide,
            commands::agent_authenticate,
            commands::export_source
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Schematlas")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                tauri::async_runtime::block_on(app.state::<Arc<agents::AgentHub>>().shutdown());
            }
        });
}
