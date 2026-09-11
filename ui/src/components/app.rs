//! Application shell: sidebar, main area, agent panel, toasts and modals.
use super::agent::{AgentPanel, ResizablePanel};
use super::dialogs::{
    action, ApiDialog, ConfirmDialog, ConnectDialog, ImportDialog, ProjectDialog,
};
use super::empty_state::EmptyState;
use super::icons::Icon;
use super::sidebar::Sidebar;
use super::source_workspace::SourceWorkspace;
use crate::api;
use crate::state::Workspace;
use crate::types::{AgentSnapshot, Position, Project};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modal {
    Api,
    Create,
    Edit,
    Connect,
    Reconnect,
    Import,
    DeleteProject,
    DeleteSource,
}

const TOAST: &str = "fixed bottom-[50px] left-1/2 z-[1000] flex w-max max-w-[640px] -translate-x-1/2 animate-rise-in items-center gap-3 rounded-lg border border-line bg-surface py-3.5 pr-[15px] pl-[18px] text-soft shadow-[0_7px_30px_#0006]";

#[component]
pub fn App() -> impl IntoView {
    let workspace = Workspace::new();
    provide_context(workspace);
    provide_context(crate::appearance::Appearance::new());
    let modal = RwSignal::new(None::<Modal>);
    let agent_open = RwSignal::new(false);
    let refreshing = RwSignal::new(false);
    let demo_busy = RwSignal::new(false);
    let desktop = api::desktop();
    let agent_project = Memo::new(move |_| {
        if !agent_open.get() {
            return None;
        }
        workspace
            .project
            .with(|p| p.as_ref().map(|p| (p.id.clone(), p.name.clone())))
    });
    spawn_local(async move { workspace.load().await });
    if desktop {
        let projects = api::listen::<Project>("workspace:update", move |project| {
            workspace.replace(project)
        });
        let agents = api::listen::<AgentSnapshot>("agent:update", move |snapshot| {
            if snapshot.reviews.is_empty() {
                return;
            }
            if workspace.project_id.get_untracked().as_deref() == Some(snapshot.project_id.as_str())
            {
                agent_open.set(true);
            } else {
                let name = workspace
                    .projects
                    .with_untracked(|list| {
                        list.iter()
                            .find(|p| p.id == snapshot.project_id)
                            .map(|p| p.name.clone())
                    })
                    .unwrap_or_else(|| "another project".into());
                workspace.notice.set(format!(
                    "Agent waiting for review in {name}. Open that project’s Local agent panel."
                ));
            }
        });
        // Stored on this owner: dropping the component drops the listeners, which unsubscribe.
        let _listeners = StoredValue::new_local((projects, agents));
    }
    let open = move |which: Modal| Callback::new(move |_: ()| modal.set(Some(which)));
    let close = Callback::new(move |_: ()| modal.set(None));
    let demo = Callback::new(move |_: ()| {
        spawn_local(async move {
            demo_busy.set(true);
            match api::create_demo().await {
                Ok(project) => {
                    let first = project.sources.first().map(|s| s.id.clone());
                    workspace.upsert(project, false);
                    workspace.source_id.set(first);
                }
                Err(e) => workspace.fail(e),
            }
            demo_busy.set(false);
        });
    });
    let current = move || {
        let project = workspace.project.get_untracked()?;
        let source = workspace.source.get_untracked()?;
        Some((project.id.clone(), source))
    };
    let refresh = Callback::new(move |_: ()| {
        let Some((project_id, source)) = current() else {
            return;
        };
        spawn_local(async move {
            refreshing.set(true);
            match api::refresh_source(&project_id, &source.id).await {
                Ok(project) => {
                    workspace.upsert(project, false);
                    workspace.notice.set("Schema refreshed.".into());
                }
                Err(e) => {
                    if e.contains("Reconnect") {
                        modal.set(Some(Modal::Reconnect));
                    }
                    workspace.fail(e);
                }
            }
            refreshing.set(false);
        });
    });
    let save_positions = Callback::new(move |positions: HashMap<String, Position>| {
        let Some((project_id, source)) = current() else {
            return;
        };
        spawn_local(async move {
            match api::save_positions(&project_id, &source.id, &positions).await {
                Ok(project) => workspace.replace(project),
                Err(e) => workspace.fail(e),
            }
        });
    });
    let export = Callback::new(move |_: ()| {
        let Some((project_id, source)) = current() else {
            return;
        };
        spawn_local(async move {
            let result = if api::desktop() {
                let safe: String = source
                    .name
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                            c
                        } else {
                            '-'
                        }
                    })
                    .collect();
                match api::save_dialog(
                    "Export schema map",
                    &format!("{safe}.json"),
                    "JSON",
                    &["json"],
                )
                .await
                {
                    Ok(Some(path)) => api::export_source(&project_id, &source.id, &path)
                        .await
                        .map(|_| true),
                    Ok(None) => Ok(false),
                    Err(e) => Err(e),
                }
            } else {
                serde_json::to_string_pretty(&*source)
                    .map_err(|e| e.to_string())
                    .and_then(|json| api::download_json("schema-map.json", &json))
                    .map(|_| false)
            };
            match result {
                Ok(true) => workspace.notice.set("Schema map exported.".into()),
                Ok(false) => {}
                Err(e) => workspace.fail(e),
            }
        });
    });
    let source_key = Memo::new(move |_| {
        Some(format!(
            "{}:{}",
            workspace.project.get()?.id,
            workspace.source.get()?.id
        ))
    });
    let upsert = Callback::new(move |project: Project| workspace.upsert(project, false));
    let upsert_last = Callback::new(move |project: Project| workspace.upsert(project, true));

    view! {
        <Show when=move || desktop>
            <div class="fixed inset-x-0 top-0 z-[100] h-[30px] bg-sidebar" data-tauri-drag-region></div>
        </Show>
        <div class="flex min-h-[620px] overflow-hidden bg-bg" class=("relative", move || desktop) class=("mt-[30px]", move || desktop) style:height=if desktop { "calc(100dvh - 30px)" } else { "100dvh" }>
            <Sidebar
                on_agent=Callback::new(move |_: ()| agent_open.update(|open| *open = !*open))
                on_create=open(Modal::Create)
                on_connect=open(Modal::Connect)
                on_import=open(Modal::Import)
                on_edit=open(Modal::Edit)
                on_home=Callback::new(move |_: ()| workspace.source_id.set(None))
            />
            <main class="flex min-w-0 flex-1 flex-col">
                <Show when=move || !desktop>
                    <div class="shrink-0 border-b border-accent-line bg-accent-soft p-1.5 text-center text-[10px] font-semibold text-accent-text">
                        "Browser preview " <span class="ml-2.5 font-normal text-faint">"Database connections and OpenAPI imports run in the desktop app."</span>
                    </div>
                </Show>
                {move || {
                    if workspace.loading.get() {
                        view! {
                            <div class="flex flex-1 flex-col items-center justify-center gap-[18px] text-[13px] text-muted">
                                <span class="size-[25px] animate-spin rounded-full border-2 border-line-strong border-t-accent"></span>
                                <p>"Opening your workspace…"</p>
                            </div>
                        }.into_any()
                    } else if source_key.get().is_some() {
                        view! {
                            <SourceWorkspace
                                on_api=open(Modal::Api)
                                on_refresh=refresh
                                on_reconnect=open(Modal::Reconnect)
                                on_export=export
                                on_remove=open(Modal::DeleteSource)
                                on_save=save_positions
                                refreshing=refreshing
                            />
                        }.into_any()
                    } else {
                        let has_project = workspace.project.with(|p| p.is_some());
                        let name = workspace.project.with(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default());
                        view! {
                            <div class="flex h-20 items-center justify-between border-b border-line px-10 max-[1000px]:px-[30px]">
                                <span class="text-[10px] tracking-[1.6px] text-muted">{if has_project { "PROJECT OVERVIEW" } else { "YOUR WORKSPACE" }}</span>
                                <Show when=move || has_project>
                                    <div class="flex gap-2.5">
                                        <button type="button" class="btn" on:click=move |_| modal.set(Some(Modal::Import))><Icon name="braces" size=15 /> " Import OpenAPI"</button>
                                        <button type="button" class="btn btn-primary" on:click=move |_| modal.set(Some(Modal::Connect))><Icon name="plus" size=15 /> " Connect database"</button>
                                    </div>
                                </Show>
                            </div>
                            <EmptyState
                                has_project=has_project
                                name=name
                                on_create=open(Modal::Create)
                                on_connect=open(Modal::Connect)
                                on_import=open(Modal::Import)
                                on_demo=demo
                                busy=demo_busy
                            />
                        }.into_any()
                    }
                }}
            </main>
            {move || {
                match agent_project.get() {
                    Some((id, name)) => view! {
                        <ResizablePanel>
                            <AgentPanel project_id=id project_name=name on_close=Callback::new(move |_: ()| agent_open.set(false)) />
                        </ResizablePanel>
                    }.into_any(),
                    _ => ().into_any(),
                }
            }}
        </div>
        <Show when=move || !workspace.error.get().is_empty()>
            <div class=format!("{TOAST} border-danger-line text-ink") role="alert">
                <Icon name="circle-alert" size=18 />
                <p class="max-w-[540px] text-xs leading-relaxed [overflow-wrap:anywhere]">{move || workspace.error.get()}</p>
                <button type="button" class="icon-btn" aria-label="Dismiss error" on:click=move |_| workspace.error.set(String::new())><Icon name="x" size=17 /></button>
            </div>
        </Show>
        <Show when=move || workspace.error.get().is_empty() && !workspace.notice.get().is_empty()>
            <div class=TOAST role="status">
                <Icon name="circle-check-big" size=18 />
                <p class="max-w-[540px] text-xs leading-relaxed [overflow-wrap:anywhere]">{move || workspace.notice.get()}</p>
                <button type="button" class="icon-btn" aria-label="Dismiss notification" on:click=move |_| workspace.notice.set(String::new())><Icon name="x" size=17 /></button>
            </div>
        </Show>
        {move || {
            let project = workspace.project.get();
            let source = workspace.source.get();
            match modal.get() {
                Some(Modal::Create) => view! { <ProjectDialog project=None on_close=close on_save=upsert on_delete=open(Modal::DeleteProject) /> }.into_any(),
                Some(Modal::Edit) => view! { <ProjectDialog project=project on_close=close on_save=upsert on_delete=open(Modal::DeleteProject) /> }.into_any(),
                Some(which @ (Modal::Connect | Modal::Reconnect)) => match project {
                    Some(project) => view! {
                        <ConnectDialog
                            project_id=project.id.clone()
                            source=if which == Modal::Reconnect { source } else { None }
                            on_close=close
                            on_save=if which == Modal::Connect { upsert_last } else { upsert }
                        />
                    }.into_any(),
                    None => ().into_any(),
                },
                Some(Modal::Import) => match project {
                    Some(project) => view! { <ImportDialog project_id=project.id.clone() on_close=close on_save=upsert_last /> }.into_any(),
                    None => ().into_any(),
                },
                Some(Modal::DeleteProject) => match project {
                    Some(project) => view! {
                        <ConfirmDialog
                            title="Delete this project?"
                            message=format!("Delete “{}” and its saved maps from this workspace? Your databases and original files are unaffected.", project.name)
                            on_close=close
                            on_confirm=action(move || workspace.delete_project())
                        />
                    }.into_any(),
                    None => ().into_any(),
                },
                Some(Modal::DeleteSource) => match (project, source) {
                    (Some(project), Some(source)) => {
                        let (project_id, source_id) = (project.id.clone(), source.id.clone());
                        view! {
                            <ConfirmDialog
                                title="Delete this source?"
                                message=format!("Remove “{}” and its saved map from this project? Your database or original file is unaffected.", source.name)
                                on_close=close
                                on_confirm=action(move || {
                                    let (project_id, source_id) = (project_id.clone(), source_id.clone());
                                    async move {
                                        let updated = api::remove_source(&project_id, &source_id).await?;
                                        workspace.upsert(updated, false);
                                        Ok(())
                                    }
                                })
                            />
                        }.into_any()
                    }
                    _ => ().into_any(),
                },
                Some(Modal::Api) => match (project, source) {
                    (Some(project), Some(source)) => view! { <ApiDialog project_id=project.id.clone() source=source on_close=close on_save=upsert /> }.into_any(),
                    _ => ().into_any(),
                },
                None => ().into_any(),
            }
        }}
    }
}
