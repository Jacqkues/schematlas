pub mod agents;
pub mod atlas;
pub mod canvas;
mod commands;
pub mod connectors;
pub mod dbml_export;
pub mod documents;
pub mod domain;
pub mod error;
pub mod execution;
#[path = "../../shared/explorer.rs"]
pub mod explorer;
pub mod freshness;
pub mod openapi;
mod project_folders;
pub mod repo;
pub mod repository;
pub mod service;
pub mod sync;
#[path = "../../shared/toc.rs"]
pub mod toc;
pub mod toc_service;
pub mod wiki;
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
            let sync = sync::SyncHub::new(workspace.clone());
            let mut sync_updates = sync.updates.subscribe();
            let sync_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut pending = std::collections::HashMap::new();
                let mut frame = tokio::time::interval(std::time::Duration::from_millis(100));
                frame.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    tokio::select! {
                        update = sync_updates.recv() => match update {
                            Ok(snapshot) => { pending.insert(snapshot.project_id.clone(), snapshot); }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        },
                        _ = frame.tick() => {
                            for (_, snapshot) in pending.drain() { let _ = sync_handle.emit("sync:update", snapshot); }
                        }
                    }
                }
            });
            app.manage(sync);
            let mut atlas = workspace.atlas_events.subscribe();
            let atlas_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match atlas.recv().await {
                        Ok(project_id) => {
                            let _ = atlas_handle.emit("repo:update", project_id);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            let mut navigation = hub.navigation.subscribe();
            let navigation_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match navigation.recv().await {
                        Ok(request) => {
                            let _ = navigation_handle.emit("atlas:navigate", request);
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
            commands::agent_connect_installed,
            commands::agent_working_directory,
            commands::agent_status,
            commands::agent_conversations,
            commands::agent_reopen_conversation,
            commands::agent_set_model,
            commands::agent_set_mode,
            commands::agent_prompt,
            commands::agent_cancel,
            commands::agent_new_conversation,
            commands::agent_disconnect,
            commands::agent_decide,
            commands::agent_authenticate,
            commands::export_source,
            commands::open_repository,
            commands::list_repo,
            commands::get_toc,
            commands::create_toc_doc,
            commands::delete_toc_doc,
            commands::edit_toc,
            commands::get_toc_doc,
            commands::save_toc_doc,
            commands::get_diagram,
            commands::edit_diagram,
            commands::restore_diagram,
            commands::reset_diagram_layout,
            commands::list_branches,
            commands::set_reference_branch,
            commands::diagram_freshness,
            commands::review_node,
            commands::repo_freshness,
            commands::create_diagram,
            commands::export_dbml,
            commands::delete_diagram,
            commands::get_doc,
            commands::save_doc,
            commands::delete_doc,
            commands::code_snippet,
            commands::open_code_ref,
            commands::sync_status,
            commands::sync_preview,
            commands::sync_enable,
            commands::sync_disable,
            commands::sync_post,
            commands::sync_now
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Schematlas")
        .run(|app, event| match event {
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::Focused(focused),
                ..
            } => app.state::<Arc<sync::SyncHub>>().set_focus(focused),
            tauri::RunEvent::Exit => {
                tauri::async_runtime::block_on(app.state::<Arc<sync::SyncHub>>().shutdown());
                tauri::async_runtime::block_on(app.state::<Arc<agents::AgentHub>>().shutdown());
            }
            _ => {}
        });
}
