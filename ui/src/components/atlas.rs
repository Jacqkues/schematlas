//! The repository atlas: the overview of `.schematlas/`, the diagram editor with its inspector,
//! Markdown docs, and the dialogs that create them. Files in the repository are the source of
//! truth; every save is compare-and-swap against the hash last read.
use super::dialogs::{FormError, Modal};
use super::graph::diagram_canvas::DiagramCanvas;
use super::icons::Icon;
use super::team::{announce, comment_counts, open_comments, Avatar};
use crate::api;
use crate::diagram_layout::{apply, Edit};
use crate::markdown::render_markdown;
use crate::state::{AtlasItem, Workspace};
use crate::types::{CodeRef, Diagram, DiagramEdge, DiagramFile, DiagramNode, DocFile, Snippet};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const NODE_KINDS: [&str; 12] = [
    "service", "module", "store", "external", "client", "worker", "queue", "actor", "decision",
    "doc", "library", "config",
];
const DIAGRAM_KINDS: [&str; 7] = [
    "architecture",
    "flow",
    "data",
    "sequence",
    "modules",
    "deployment",
    "freeform",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Create {
    Diagram,
    Doc,
    Mermaid,
}

fn conflict(error: &str) -> bool {
    error.contains("changed since")
}
fn slug_of(title: &str) -> String {
    let mut slug: String = title
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug: String = slug.trim_matches('-').chars().take(60).collect();
    if slug.is_empty() {
        "untitled".into()
    } else {
        slug
    }
}
fn unused(slug: String, taken: impl Fn(&str) -> bool) -> String {
    let mut candidate = slug.clone();
    let mut n = 2;
    while taken(&candidate) {
        candidate = format!("{slug}-{n}");
        n += 1;
    }
    candidate
}

/// Prepare a request without sending it before the user has connected an agent.
pub fn ask_agent(workspace: Workspace, agent_open: RwSignal<bool>, prompt: String) {
    workspace.agent_draft.update(|draft| {
        if !draft.trim().is_empty() {
            draft.push_str("\n\n");
        }
        draft.push_str(&prompt);
    });
    agent_open.set(true);
}

#[component]
pub fn AtlasOverview(
    agent_open: RwSignal<bool>,
    #[prop(into)] on_create: Callback<Create>,
    #[prop(into)] on_connect: Callback<()>,
    #[prop(into)] on_import: Callback<()>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let desktop = api::desktop();
    let busy = RwSignal::new(false);
    let has_repo = move || workspace.repo.with(|r| r.root.is_some());
    let choose_repo = move |_| {
        let Some(project) = workspace.project.get_untracked() else {
            return;
        };
        spawn_local(async move {
            busy.set(true);
            let chosen = api::open_dialog(
                "Open a repository",
                true,
                project.working_directory.as_deref(),
            )
            .await;
            match chosen {
                Ok(Some(path)) => match api::open_repository(&project.id, &path).await {
                    Ok(updated) => {
                        workspace.replace(updated);
                        workspace.refresh_repo().await;
                        workspace.refresh_sync().await;
                    }
                    Err(e) => workspace.fail(e),
                },
                Ok(None) => {}
                Err(e) => workspace.fail(e),
            }
            busy.set(false);
        });
    };
    let prompt = RwSignal::new(String::new());
    let prepare = move |text: String| {
        if !text.trim().is_empty() {
            ask_agent(workspace, agent_open, text.trim().to_string());
            prompt.set(String::new());
        }
    };
    let card = "group flex min-h-[136px] flex-col gap-3 rounded-2xl border border-line bg-surface p-5 text-left transition-colors hover:border-line-strong hover:bg-surface-3";
    view! {
        <header class="flex min-h-16 shrink-0 items-center justify-between gap-4 px-8 py-3">
            <div class="min-w-0">
                <h1 class="truncate text-sm font-medium">{move || workspace.project.with(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default())}</h1>
                <p class="mt-1 truncate text-[11px] text-muted" title=move || workspace.repo.with(|r| r.root.clone().unwrap_or_default())>
                    {move || if !desktop { "Example repository · Browser preview".to_string() } else { workspace.repo.with(|r| r.root.as_deref().and_then(|p| p.rsplit('/').find(|s| !s.is_empty())).map(|s| format!("{s} · Repository connected")).unwrap_or_else(|| "Your documentation workspace".into())) }}
                </p>
            </div>
            <Show when=move || desktop>
                <button type="button" class="btn bg-transparent" disabled=move || busy.get() on:click=choose_repo><Icon name="folder-git" size=15 /> {move || if has_repo() { "Change repository" } else { "Open repository" }}</button>
            </Show>
        </header>
        <div class="min-h-0 flex-1 overflow-auto">
            <div class="mx-auto w-full max-w-[1080px] px-8 pb-12">
                <section class="mx-auto max-w-[680px] pt-[clamp(32px,6vh,76px)] pb-12 text-center">
                    <h2 class="text-[clamp(24px,2.5vw,34px)] font-medium tracking-[-1px]">"What would you like to understand?"</h2>
                    <p class="mt-3 text-[13px] leading-6 text-muted">"Explore the architecture. Follow a flow. Keep the explanation with the code."</p>
                    <Show when=has_repo fallback=move || view! {
                        <div class="mt-7 rounded-2xl border border-line bg-surface p-6">
                            <Icon name="folder-git" size=24 class="text-sage" />
                            <h3 class="mt-3 text-sm font-medium">"Start with your repository"</h3>
                            <p class="mt-2 text-xs leading-6 text-muted">"Your agent reads the code and proposes maps and documentation for you to review."</p>
                            <Show when=move || desktop fallback=|| view! { <p class="mt-4 text-xs text-muted">"Open the desktop app to choose a repository."</p> }>
                                <button type="button" class="btn btn-primary mt-5" disabled=move || busy.get() on:click=choose_repo><Icon name="folder-open" size=15 /> "Open repository"</button>
                            </Show>
                        </div>
                    }>
                        <form class="mt-7 rounded-[24px] border border-line bg-surface-3 p-4 text-left shadow-[0_4px_20px_#00000008] focus-within:border-line-strong" on:submit=move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); prepare(prompt.get_untracked()); }>
                            <label class="sr-only" for="atlas-prompt">"What would you like to understand?"</label>
                            <textarea id="atlas-prompt" class="min-h-[66px] w-full resize-none border-0 bg-transparent px-1 py-2 text-sm leading-6 text-ink outline-none placeholder:text-muted focus-visible:outline-none" placeholder="Ask about your codebase…" bind:value=prompt maxlength="64000" rows="2"></textarea>
                            <div class="flex items-center justify-between gap-3">
                                <span class="flex items-center gap-2 px-1 text-xs text-muted"><Icon name="folder-git" size=14 /> "Your repository"</span>
                                <button type="submit" class="btn btn-primary size-9 min-h-0 rounded-full p-0" aria-label="Prepare agent request" title="Continue with your agent" disabled=move || prompt.with(|p| p.trim().is_empty())><Icon name="arrow-up" size=17 /></button>
                            </div>
                        </form>
                        <div class="mt-4 flex flex-wrap justify-center gap-2">
                            {[
                                ("workflow", "Map the architecture", "Map this repository: propose a focused architecture diagram of the main components and their connections, with code references and a short overview document."),
                                ("spline", "Explain a flow", "Help me choose a critical flow in this repository, then explain it step by step with a diagram and code references."),
                                ("file-text", "Write an overview", "Create a concise onboarding overview of this repository, backed by code references and links to its diagrams.")
                            ].into_iter().map(move |(icon, label, request)| view! {
                                <button type="button" class="flex items-center gap-2 rounded-full border border-line px-3.5 py-2.5 text-xs text-soft hover:bg-surface-3 hover:text-ink" on:click=move |_| prepare(request.into())><Icon name=icon size=14 />{label}</button>
                            }).collect_view()}
                        </div>
                        <p class="mt-4 text-[11px] text-faint">"Use your installed coding agent. Review changes before they are saved."</p>
                    </Show>
                </section>
                <Show when=has_repo>
                    <section class="mb-9" aria-label="Diagrams">
                        <div class="mb-4 flex items-center justify-between gap-3"><h2 class="text-sm font-medium">"Diagrams"</h2><div class="flex gap-2"><button type="button" class="btn min-h-8 bg-transparent px-3 py-1.5 text-[11px]" on:click=move |_| on_create.run(Create::Mermaid)>"Import Mermaid"</button><button type="button" class="btn min-h-8 px-3 py-1.5 text-[11px]" on:click=move |_| on_create.run(Create::Diagram)><Icon name="plus" size=13 /> "New diagram"</button></div></div>
                        <div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
                            <For each=move || workspace.repo.with(|r| r.diagrams.clone()) key=|d| (d.slug.clone(), d.hash.clone()) children=move |entry| {
                                let slug = entry.slug.clone();
                                view! { <button type="button" class=card on:click=move |_| workspace.open_item(AtlasItem::Diagram(slug.clone()))>
                                    <span class="flex items-center justify-between text-muted"><Icon name="workflow" size=20 /><Icon name="arrow-up-right" size=15 class="opacity-0 transition-opacity group-hover:opacity-100" /></span>
                                    <strong class="text-[13px] font-medium">{entry.title.clone()}</strong>
                                    {match entry.error.clone() { Some(e) => view! { <span class="text-xs text-danger-text">{e}</span> }.into_any(), None => view! { <span class="mt-auto text-[11px] text-muted">{format!("{} components · {} connections", entry.nodes, entry.edges)}</span> }.into_any() }}
                                </button> }
                            } />
                            <Show when=move || workspace.repo.with(|r| r.diagrams.is_empty())><button type="button" class=format!("{card} items-center justify-center border-dashed text-muted") on:click=move |_| on_create.run(Create::Diagram)><Icon name="plus" size=20 /><span class="text-xs">"Start a blank diagram"</span></button></Show>
                        </div>
                    </section>
                    <section class="mb-9" aria-label="Documents">
                        <div class="mb-3 flex items-center justify-between"><h2 class="text-sm font-medium">"Documents"</h2><button type="button" class="icon-btn" aria-label="New doc" title="New document" on:click=move |_| on_create.run(Create::Doc)><Icon name="plus" size=16 /></button></div>
                        <div class="divide-y divide-line-soft">
                            <For each=move || workspace.repo.with(|r| r.docs.clone()) key=|d| (d.slug.clone(), d.hash.clone()) children=move |entry| {
                                let slug = entry.slug.clone();
                                view! { <button type="button" class="flex w-full items-center gap-3 rounded-lg px-3 py-4 text-left hover:bg-surface-3" on:click=move |_| workspace.open_item(AtlasItem::Doc(slug.clone()))><Icon name="file-text" size=17 class="text-muted" /><span class="flex-1 text-[13px]">{entry.title}</span><Icon name="arrow-right" size=14 class="text-muted" /></button> }
                            } />
                            <Show when=move || workspace.repo.with(|r| r.docs.is_empty())><p class="py-3 text-xs text-muted">"Write a document, or ask your agent to get one started."</p></Show>
                        </div>
                    </section>
                </Show>
                <section class="flex flex-wrap items-center gap-3 border-t border-line-soft pt-5" aria-label="Data sources">
                    <p class="mr-auto text-xs text-muted">"Add context from your data"</p>
                    <button type="button" class="btn bg-transparent" on:click=move |_| on_connect.run(())><Icon name="database" size=14 /> "Connect database"</button>
                    <button type="button" class="btn bg-transparent" on:click=move |_| on_import.run(())><Icon name="braces" size=14 /> "Import OpenAPI"</button>
                </section>
            </div>
        </div>
    }
}

#[component]
pub fn CreateDialog(which: Create, #[prop(into)] on_close: Callback<()>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let default_folder = if which == Create::Doc {
        "docs"
    } else {
        "diagrams"
    };
    let destination = RwSignal::new(workspace.project.with_untracked(|p| {
        p.as_ref()
            .and_then(|p| p.explorer.location("", default_folder))
            .unwrap_or_default()
    }));
    let title = RwSignal::new(String::new());
    let kind = RwSignal::new("architecture".to_string());
    let mermaid = RwSignal::new(
        "flowchart LR\n  web[Web app] -->|HTTP| api(API)\n  api --> db[(Database)]".to_string(),
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(project_id) = workspace.project_id.get_untracked() else {
            return;
        };
        let name = title.get_untracked().trim().to_string();
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let result = match which {
                Create::Diagram => {
                    let slug = workspace.repo.with_untracked(|r| {
                        unused(slug_of(&name), |s| r.diagrams.iter().any(|d| d.slug == s))
                    });
                    let diagram = Diagram {
                        version: 1,
                        kind: kind.get_untracked(),
                        title: name.clone(),
                        ..Diagram::default()
                    };
                    api::save_diagram(&project_id, &slug, &diagram, None)
                        .await
                        .map(|f| AtlasItem::Diagram(f.slug))
                }
                Create::Mermaid => {
                    api::import_mermaid(&project_id, &name, &mermaid.get_untracked())
                        .await
                        .map(|f| AtlasItem::Diagram(f.slug))
                }
                Create::Doc => {
                    let slug = workspace.repo.with_untracked(|r| {
                        unused(slug_of(&name), |s| r.docs.iter().any(|d| d.slug == s))
                    });
                    api::save_doc(&project_id, &slug, &format!("# {name}\n\n"), None)
                        .await
                        .map(|f| AtlasItem::Doc(f.slug))
                }
            };
            match result {
                Ok(item) => {
                    let folder = destination.get_untracked();
                    let reference = match &item {
                        AtlasItem::Diagram(slug) => format!("diagram:{slug}"),
                        AtlasItem::Doc(slug) => format!("doc:{slug}"),
                    };
                    let desired = (!folder.is_empty()).then_some(folder);
                    let current = workspace.project.with_untracked(|p| {
                        p.as_ref()
                            .and_then(|p| p.explorer.location(&reference, default_folder))
                    });
                    if desired != current {
                        match api::edit_explorer(
                            &project_id,
                            &schematlas_ui::explorer::Edit::Move {
                                item: reference,
                                folder: desired,
                            },
                        )
                        .await
                        {
                            Ok(project) => workspace.replace(project),
                            Err(e) => workspace.fail(format!(
                                "The file was created, but its folder could not be saved: {e}"
                            )),
                        }
                    }
                    if workspace.project_id.get_untracked().as_deref() == Some(&project_id) {
                        workspace.refresh_repo().await;
                        workspace.open_item(item);
                    }
                    on_close.run(());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    let (heading, subtitle) = match which {
        Create::Diagram => (
            "New diagram",
            "Saved as .schematlas/diagrams/<name>.json in the repository.",
        ),
        Create::Doc => (
            "New doc",
            "Saved as .schematlas/docs/<name>.md in the repository.",
        ),
        Create::Mermaid => (
            "Import Mermaid",
            "Paste a flowchart. Subgraphs become groups; styling is ignored.",
        ),
    };
    view! {
        <Modal title=heading subtitle=subtitle on_close=on_close busy=busy>
            <form on:submit=submit>
                <label class="form-label" for="atlas-title">"Title " <span class="text-accent">"*"</span></label>
                <input id="atlas-title" class="field mb-[21px]" bind:value=title placeholder="e.g. Checkout flow" required maxlength="120" />
                <super::explorer::FolderSelect value=destination />
                {(which == Create::Diagram).then(|| view! {
                    <label class="form-label" for="atlas-kind">"Kind"</label>
                    <select id="atlas-kind" class="field mb-[21px]" on:change=move |ev| kind.set(event_target_value(&ev))>
                        {DIAGRAM_KINDS.iter().map(|k| view! { <option value=*k>{*k}</option> }).collect_view()}
                    </select>
                })}
                {(which == Create::Mermaid).then(|| view! {
                    <label class="form-label" for="atlas-mermaid">"Mermaid flowchart"</label>
                    <textarea id="atlas-mermaid" class="field mb-[21px] min-h-[180px] font-mono" bind:value=mermaid spellcheck="false"></textarea>
                })}
                <FormError error=error />
                <div class="modal-footer">
                    <span class="form-hint">"Commit it with your code to share it."</span>
                    <button type="submit" class="btn btn-primary" disabled=move || busy.get()>
                        {move || if busy.get() { "Creating…" } else { "Create" }} <Icon name="arrow-right" size=16 />
                    </button>
                </div>
            </form>
        </Modal>
    }
}

#[component]
fn CodeRefs(refs: Vec<CodeRef>, #[prop(into)] on_remove: Callback<usize>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let shown = RwSignal::new(None::<(usize, Result<Snippet, String>)>);
    let open = move |reference: String| {
        let Some(project_id) = workspace.project_id.get_untracked() else {
            return;
        };
        spawn_local(async move {
            if let Err(e) = api::open_code_ref(&project_id, &reference).await {
                workspace.fail(e);
            }
        });
    };
    let peek = move |index: usize, reference: String| {
        if shown.with_untracked(|s| s.as_ref().is_some_and(|(i, _)| *i == index)) {
            shown.set(None);
            return;
        }
        let Some(project_id) = workspace.project_id.get_untracked() else {
            return;
        };
        spawn_local(async move {
            let result = api::code_snippet(&project_id, &reference).await;
            shown.set(Some((index, result)));
        });
    };
    view! {
        <ul class="flex flex-col gap-1.5">
            {refs.into_iter().enumerate().map(|(index, code_ref)| {
                let reference = code_ref.reference();
                let peek_ref = reference.clone();
                let open_ref = reference.clone();
                view! {
                    <li class="rounded-md border border-line bg-surface-3">
                        <div class="flex items-center gap-1 px-2 py-1.5">
                            <button type="button" class="min-w-0 flex-1 truncate text-left font-mono text-[11px] text-accent-text hover:underline" title="Show the code" on:click=move |_| peek(index, peek_ref.clone())>
                                {code_ref.label()}
                            </button>
                            <Show when=api::desktop>
                                {let open_ref = open_ref.clone(); view! {
                                    <button type="button" class="icon-btn size-6" aria-label="Open in editor" title="Open in editor" on:click=move |_| open(open_ref.clone())><Icon name="arrow-up-right" size=13 /></button>
                                }}
                            </Show>
                            <button type="button" class="icon-btn size-6" aria-label="Remove reference" on:click=move |_| on_remove.run(index)><Icon name="x" size=13 /></button>
                        </div>
                        {move || shown.get().filter(|(i, _)| *i == index).map(|(_, result)| match result {
                            Ok(snippet) => {
                                let focus = snippet.focus;
                                view! {
                                    <pre class="max-h-64 overflow-auto border-t border-line bg-field py-1.5 font-mono text-[10px] leading-[1.6]">
                                        {snippet.lines.iter().enumerate().map(|(i, line)| {
                                            let number = snippet.start + i as u32;
                                            let hot = focus.is_some_and(|(a, b)| number >= a && number <= b);
                                            view! {
                                                <div class="flex gap-3 px-2" class=("bg-accent-soft", hot)>
                                                    <span class="w-8 shrink-0 text-right text-faint select-none">{number}</span>
                                                    <span class="whitespace-pre text-text">{line.clone()}</span>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </pre>
                                }.into_any()
                            }
                            Err(e) => view! { <p class="border-t border-line p-2 text-[10px] text-danger-text">{e}</p> }.into_any(),
                        })}
                    </li>
                }
            }).collect_view()}
        </ul>
    }
}

#[component]
fn NodeInspector(
    slug: String,
    node: DiagramNode,
    diagram: Arc<Diagram>,
    #[prop(into)] on_change: Callback<DiagramNode>,
    #[prop(into)] on_edges: Callback<Vec<DiagramEdge>>,
    #[prop(into)] on_delete: Callback<String>,
    #[prop(into)] on_explain: Callback<DiagramNode>,
    #[prop(into)] on_navigate: Callback<String>,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let current = StoredValue::new(node.clone());
    let commit = move |edit: &dyn Fn(&mut DiagramNode)| {
        let mut next = current.get_value();
        edit(&mut next);
        if next != current.get_value() {
            current.set_value(next.clone());
            on_change.run(next);
        }
    };
    let new_ref = RwSignal::new(String::new());
    let comment = RwSignal::new(String::new());
    let id = node.id.clone();
    let labels: HashMap<String, String> = diagram
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.label.clone()))
        .collect();
    let connections: Vec<(usize, DiagramEdge)> = diagram
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.source == id || e.target == id)
        .map(|(i, e)| (i, e.clone()))
        .collect();
    let edges = StoredValue::new(diagram.edges.clone());
    let update_edge = move |index: usize, label: Option<String>, remove: bool| {
        let mut list = edges.get_value();
        if remove {
            list.remove(index);
        } else if let Some(edge) = list.get_mut(index) {
            edge.label = label.filter(|l| !l.trim().is_empty());
        }
        edges.set_value(list.clone());
        on_edges.run(list);
    };
    let thread_slug = slug.clone();
    let thread_id = id.clone();
    let thread = Memo::new(move |_| {
        workspace.sync.with(|s| {
            s.as_ref()
                .map(|s| {
                    open_comments(&s.events, &thread_slug)
                        .into_iter()
                        .filter(|c| c.body["node"].as_str() == Some(thread_id.as_str()))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let syncing = move || {
        workspace
            .sync
            .with(|s| s.as_ref().is_some_and(|s| s.enabled))
    };
    let post_slug = slug.clone();
    let post_id = id.clone();
    let post = move || {
        let text = comment.get_untracked().trim().to_string();
        if text.is_empty() {
            return;
        }
        announce(
            workspace,
            "comment",
            json!({"diagram": post_slug.clone(), "node": post_id.clone(), "text": text}),
        );
        comment.set(String::new());
    };
    let delete_id = id.clone();
    let explain_node = node.clone();
    let no_description = node.body_md.as_ref().is_none_or(|b| b.trim().is_empty());
    view! {
        <aside class="absolute top-0 right-0 bottom-0 z-20 flex w-[340px] animate-rise-in flex-col border-l border-line bg-surface-2 shadow-[-12px_0_35px_#0005]" aria-label="Node details" on:keydown=|ev| ev.stop_propagation()>
            <header class="flex items-center justify-between border-b border-line px-4 py-3.5">
                <span class="text-xs font-medium text-muted">"Component details"</span>
                <button type="button" class="icon-btn" aria-label="Close node details" on:click=move |_| on_close.run(())><Icon name="x" size=16 /></button>
            </header>
            <div class="flex min-h-0 flex-1 flex-col gap-5 overflow-auto p-5">
                <div>
                    <span class="text-[11px] text-muted">{node.kind.clone()}</span>
                    <h2 class="mt-1 text-lg font-medium tracking-tight">{node.label.clone()}</h2>
                    {node.body_md.as_ref().filter(|b| !b.trim().is_empty()).map(|body| view! { <div class="markdown mt-4" inner_html=render_markdown(body)></div> })}
                    <Show when=move || no_description><p class="mt-3 text-xs leading-6 text-muted">"No explanation yet. Ask your agent to describe this component and link it to the code."</p></Show>
                </div>
                <details class="rounded-xl border border-line p-3">
                    <summary class="cursor-pointer text-xs font-medium text-soft">"Edit details"</summary>
                    <div class="mt-4 flex flex-col gap-4">
                <div>
                    <label class="form-label" for="node-label">"Label"</label>
                    <input id="node-label" class="field" prop:value=node.label.clone() maxlength="200"
                        on:change=move |ev| { let v = event_target_value(&ev); if !v.trim().is_empty() { commit(&|n| n.label = v.trim().to_string()); } } />
                </div>
                <div>
                    <label class="form-label" for="node-kind">"Kind"</label>
                    <input id="node-kind" class="field" list="node-kinds" prop:value=node.kind.clone() maxlength="40"
                        on:change=move |ev| { let v = event_target_value(&ev).trim().to_lowercase().replace(' ', "-"); if !v.is_empty() { commit(&|n| n.kind = v.clone()); } } />
                    <datalist id="node-kinds">{NODE_KINDS.iter().map(|k| view! { <option value=*k></option> }).collect_view()}</datalist>
                </div>
                <div>
                    <label class="form-label" for="node-body">"Description " <small class="font-normal text-muted">"Markdown"</small></label>
                    <textarea id="node-body" class="field min-h-[110px] resize-y" prop:value=node.body_md.clone().unwrap_or_default()
                        on:change=move |ev| { let v = event_target_value(&ev); commit(&|n| n.body_md = Some(v.clone()).filter(|b| !b.trim().is_empty())); }></textarea>
                </div>
                <div>
                    <label class="form-label" for="node-tags">"Tags " <small class="font-normal text-muted">"comma separated"</small></label>
                    <input id="node-tags" class="field" prop:value=node.tags.join(", ")
                        on:change=move |ev| { let v = event_target_value(&ev); commit(&|n| n.tags = v.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()); } />
                </div>
                    </div>
                </details>
                <div>
                    <div class="form-label">"Source evidence"</div>
                    <CodeRefs refs=node.code_refs.clone() on_remove=Callback::new(move |i: usize| commit(&|n| { if i < n.code_refs.len() { n.code_refs.remove(i); } })) />
                    <form class="mt-2 flex gap-1.5" on:submit=move |ev: leptos::ev::SubmitEvent| {
                        ev.prevent_default();
                        let v = new_ref.get_untracked().trim().to_string();
                        if !v.is_empty() {
                            commit(&|n| n.code_refs.push(CodeRef::Text(v.clone())));
                            new_ref.set(String::new());
                        }
                    }>
                        <input class="field py-1.5 font-mono" placeholder="src/api/mod.rs:12-80" bind:value=new_ref aria-label="Add a code reference" />
                        <button type="submit" class="btn px-2.5" aria-label="Add reference"><Icon name="plus" size=14 /></button>
                    </form>
                </div>
                <div>
                    <div class="form-label">"Connections"</div>
                    {if connections.is_empty() {
                        view! { <p class="form-hint">"None yet. Use the connect tool on the canvas."</p> }.into_any()
                    } else {
                        view! {
                            <ul class="flex flex-col gap-1.5">
                                {connections.into_iter().map(|(index, edge)| {
                                    let outgoing = edge.source == id;
                                    let other_id = if outgoing { edge.target.clone() } else { edge.source.clone() };
                                    let other = labels.get(&other_id).cloned().unwrap_or_default();
                                    view! {
                                        <li class="flex items-center gap-1.5 text-[11px]">
                                            <span class="w-4 text-faint">{if outgoing { "→" } else { "←" }}</span>
                                            <button type="button" class="w-24 truncate text-left text-ink hover:underline" title=other.clone() aria-label=format!("Go to {other}") on:click=move |_| on_navigate.run(other_id.clone())>{other.clone()}</button>
                                            <input class="field min-w-0 flex-1 py-1" placeholder="label" prop:value=edge.label.clone().unwrap_or_default()
                                                on:change=move |ev| update_edge(index, Some(event_target_value(&ev)), false) />
                                            <button type="button" class="icon-btn size-6" aria-label="Remove connection" on:click=move |_| update_edge(index, None, true)><Icon name="x" size=13 /></button>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any()
                    }}
                </div>
                <Show when=syncing>
                    <div>
                        <div class="form-label flex items-center gap-1.5"><Icon name="message-square" size=12 /> "Team comments"</div>
                        <ul class="mb-2 flex flex-col gap-2">
                            <For each=move || thread.get() key=|c| c.id.clone() children=move |c| {
                                let id = c.id.clone();
                                view! {
                                    <li class="flex gap-2 rounded-md border border-line bg-surface-3 p-2">
                                        <Avatar author=c.author.clone() size=20 />
                                        <div class="min-w-0 flex-1">
                                            <strong class="text-[11px]">{c.author.name.clone()}</strong>
                                            <p class="text-[11px] leading-relaxed whitespace-pre-wrap text-ink [overflow-wrap:anywhere]">{c.body["text"].as_str().unwrap_or_default().to_string()}</p>
                                            <button type="button" class="text-[10px] text-muted hover:text-accent" on:click=move |_| announce(workspace, "comment_resolve", json!({"commentId": id.clone()}))>"Resolve"</button>
                                        </div>
                                    </li>
                                }
                            } />
                        </ul>
                        <form class="flex gap-1.5" on:submit={let post = post.clone(); move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); post(); }}>
                            <input class="field py-1.5" placeholder="Comment for the team…" bind:value=comment aria-label="Comment on this node" />
                            <button type="submit" class="btn px-2.5" aria-label="Post comment"><Icon name="send" size=14 /></button>
                        </form>
                    </div>
                </Show>
            </div>
            <footer class="flex flex-col gap-2 border-t border-line p-3">
                <button type="button" class="btn" on:click=move |_| on_explain.run(explain_node.clone())><Icon name="sparkles" size=14 /> "Explain with agent"</button>
                <button type="button" class="btn btn-ghost-danger w-full justify-start" on:click=move |_| on_delete.run(delete_id.clone())><Icon name="trash-2" size=14 /> " Delete node"</button>
            </footer>
        </aside>
    }
}

#[component]
pub fn DiagramView(slug: String, agent_open: RwSignal<bool>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let file = RwSignal::new(None::<DiagramFile>);
    let draft = RwSignal::new(None::<Arc<Diagram>>);
    let load_error = RwSignal::new(String::new());
    let selected = RwSignal::new(None::<String>);
    let query = RwSignal::new(String::new());
    let focus_node = RwSignal::new(None::<String>);
    let history = StoredValue::new(Vec::<Diagram>::new());
    let future = StoredValue::new(Vec::<Diagram>::new());
    let editing = RwSignal::new(None::<String>);
    // The details panel opens on request and then follows the selection until closed.
    let inspecting = RwSignal::new(false);
    let saving = StoredValue::new(false);
    let again = StoredValue::new(false);
    let timer = StoredValue::new_local(None::<TimeoutHandle>);
    let announce_timer = StoredValue::new_local(None::<TimeoutHandle>);
    let confirm_delete = RwSignal::new(false);
    let project_id = workspace.project_id.get_untracked().unwrap_or_default();

    let load = {
        let (project_id, slug) = (project_id.clone(), slug.clone());
        move || {
            let (project_id, slug) = (project_id.clone(), slug.clone());
            spawn_local(async move {
                match api::get_diagram(&project_id, &slug).await {
                    Ok(f) => {
                        draft.set(Some(Arc::new(f.diagram.clone())));
                        file.set(Some(f));
                        load_error.set(String::new());
                    }
                    Err(e) => load_error.set(e),
                }
            });
        }
    };
    load();
    let save = {
        let (project_id, slug) = (project_id.clone(), slug.clone());
        let load = load.clone();
        StoredValue::new_local(move || {
            let (project_id, slug, load) = (project_id.clone(), slug.clone(), load.clone());
            if saving.get_value() {
                again.set_value(true);
                return;
            }
            let Some(current) = draft.get_untracked() else {
                return;
            };
            let base = file.with_untracked(|f| f.as_ref().map(|f| f.hash.clone()));
            saving.set_value(true);
            spawn_local(async move {
                let result = api::save_diagram(&project_id, &slug, &current, base.as_deref()).await;
                saving.set_value(false);
                match result {
                    Ok(saved) => {
                        file.set(Some(saved));
                        workspace.refresh_repo().await;
                    }
                    Err(e) if conflict(&e) => {
                        again.set_value(false);
                        workspace.notice.set("This diagram changed on disk (an agent, an editor or git). Reloaded the latest version.".into());
                        load();
                    }
                    Err(e) => {
                        again.set_value(false);
                        workspace.fail(e);
                    }
                }
            });
        })
    };
    let schedule = move || {
        timer.update_value(|t| {
            if let Some(t) = t.take() {
                t.clear();
            }
        });
        timer.set_value(
            set_timeout_with_handle(
                move || {
                    timer.set_value(None);
                    save.with_value(|s| s());
                },
                Duration::from_millis(400),
            )
            .ok(),
        );
    };
    // A queued save runs once the one in flight returns.
    Effect::new(move |_| {
        file.track();
        if again.get_value() && !saving.get_value() {
            again.set_value(false);
            save.with_value(|s| s());
        }
    });
    let note_slug = slug.clone();
    let change = Callback::new(move |(next, content): (Diagram, bool)| {
        if let Some(previous) = draft.get_untracked() {
            if *previous == next {
                return;
            }
            history.update_value(|h| {
                h.push((*previous).clone());
                if h.len() > 100 {
                    h.remove(0);
                }
            });
            future.update_value(Vec::clear);
        }
        draft.set(Some(Arc::new(next)));
        schedule();
        if content {
            let slug = note_slug.clone();
            announce_timer.update_value(|t| {
                if let Some(t) = t.take() {
                    t.clear();
                }
            });
            announce_timer.set_value(
                set_timeout_with_handle(
                    move || {
                        let title = draft
                            .with_untracked(|d| d.as_ref().map(|d| d.title.clone()))
                            .unwrap_or_default();
                        announce(
                            workspace,
                            "diagram_changed",
                            json!({"diagram": slug, "summary": format!("edited “{title}”")}),
                        );
                    },
                    Duration::from_secs(8),
                )
                .ok(),
            );
        }
    });
    // Undo and redo walk whole diagram states, so every gesture (move, rename, connect…) is one step.
    let step = move |back: bool| {
        let (from, to) = if back {
            (history, future)
        } else {
            (future, history)
        };
        let Some(state) = from.try_update_value(|h| h.pop()).flatten() else {
            return;
        };
        if let Some(current) = draft.get_untracked() {
            to.update_value(|h| h.push((*current).clone()));
        }
        editing.set(None);
        draft.set(Some(Arc::new(state)));
        schedule();
    };
    let undo = move || step(true);
    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        let target = ev
            .target()
            .and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(t).ok());
        let typing = target
            .is_some_and(|t| matches!(t.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT"));
        if typing || !(ev.meta_key() || ev.ctrl_key()) {
            return;
        }
        match ev.key().to_lowercase().as_str() {
            "z" => {
                ev.prevent_default();
                step(!ev.shift_key());
            }
            "y" => {
                ev.prevent_default();
                step(false);
            }
            _ => {}
        }
    });
    on_cleanup(move || keys.remove());

    // External changes (the agent's tools, an editor, git pull) reload when nothing is pending.
    let watch_slug = slug.clone();
    let watch_load = load.clone();
    Effect::new(move |_| {
        let disk = workspace.repo.with(|r| {
            r.diagrams
                .iter()
                .find(|d| d.slug == watch_slug)
                .map(|d| d.hash.clone())
        });
        let ours = file.with_untracked(|f| f.as_ref().map(|f| f.hash.clone()));
        let idle = !saving.get_value() && !again.get_value() && timer.with_value(Option::is_none);
        if let (Some(disk), Some(ours)) = (disk, ours) {
            if !disk.is_empty() && disk != ours && idle {
                watch_load();
            }
        }
    });
    // Presence: teammates see which diagram you have open.
    announce(workspace, "editing", json!({"diagram": slug.clone()}));
    on_cleanup(move || announce(workspace, "editing_done", json!({})));

    let diagram = Signal::derive(move || draft.get().unwrap_or_default());
    let comment_slug = slug.clone();
    let comments = Signal::derive(move || {
        workspace
            .sync
            .with(|s| comment_counts(s.as_ref(), &comment_slug))
    });
    let on_edit = Callback::new(move |edit: Edit| {
        if let Some(current) = draft.get_untracked() {
            let (next, _, content) = apply(&current, edit);
            change.run((next, content));
        }
    });
    let on_delete = Callback::new(move |id: String| {
        selected.set(None);
        on_edit.run(Edit::Delete {
            nodes: vec![id],
            edges: vec![],
        });
    });
    let on_node = Callback::new(move |node: DiagramNode| {
        let Some(current) = draft.get_untracked() else {
            return;
        };
        let mut next = (*current).clone();
        if let Some(slot) = next.nodes.iter_mut().find(|n| n.id == node.id) {
            *slot = node;
        }
        change.run((next, true));
    });
    let on_edges = Callback::new(move |edges: Vec<DiagramEdge>| {
        let Some(current) = draft.get_untracked() else {
            return;
        };
        let mut next = (*current).clone();
        next.edges = edges;
        change.run((next, true));
    });
    let rename = move |title: String| {
        let Some(current) = draft.get_untracked() else {
            return;
        };
        if title.trim().is_empty() || title == current.title {
            return;
        }
        let mut next = (*current).clone();
        next.title = title.trim().to_string();
        change.run((next, true));
    };
    let delete_slug = slug.clone();
    let delete_project = project_id.clone();
    let delete = Callback::new(move |_: ()| {
        let (project_id, slug) = (delete_project.clone(), delete_slug.clone());
        let Some(hash) = file.with_untracked(|f| f.as_ref().map(|f| f.hash.clone())) else {
            return;
        };
        spawn_local(async move {
            match api::delete_diagram(&project_id, &slug, &hash).await {
                Ok(()) => {
                    workspace.item.set(None);
                    workspace.refresh_repo().await;
                }
                Err(e) => workspace.fail(e),
            }
        });
    });
    let viewers_slug = slug.clone();
    let viewers = Memo::new(move |_| {
        workspace.sync.with(|s| {
            s.as_ref()
                .map(|s| {
                    s.presence
                        .iter()
                        .filter(|p| {
                            p.diagram.as_deref() == Some(viewers_slug.as_str())
                                && s.me.as_ref() != Some(&p.author)
                        })
                        .map(|p| p.author.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let inspector_slug = slug.clone();
    let path = format!(".schematlas/diagrams/{slug}.json");
    view! {
        <div class="flex h-full min-h-0 flex-col">
            <header class="flex min-h-[58px] shrink-0 items-center gap-3 border-b border-line bg-surface px-[18px]">
                <Icon name="workflow" size=17 class="text-sage" />
                <div class="flex min-w-0 flex-1 flex-col">
                    <input
                        class="min-w-0 truncate border-0 bg-transparent p-0 text-sm font-semibold text-ink"
                        aria-label="Diagram title"
                        prop:value=move || diagram.with(|d| d.title.clone())
                        on:change=move |ev| rename(event_target_value(&ev))
                    />
                    <span class="text-[11px] text-muted" title=path>{move || diagram.with(|d| format!("{} components · {} connections", d.nodes.len(), d.edges.len()))}</span>
                </div>
                <div class="flex -space-x-1.5">
                    <For each=move || viewers.get() key=|a| a.key.clone() children=|a| view! { <Avatar author=a size=24 /> } />
                </div>
                <search class="relative flex w-[clamp(130px,16vw,220px)] items-center gap-2 rounded-lg bg-surface-3 px-3 text-muted">
                    <Icon name="search" size=14 />
                    <input class="min-w-0 w-full border-0 bg-transparent py-[9px] text-xs text-ink" type="search" aria-label="Search nodes" placeholder="Find a component…" bind:value=query on:keydown=move |ev| { if ev.key() == "Escape" { query.set(String::new()); } } />
                    <Show when=move || !query.with(|q| q.trim().is_empty())>
                        <div class="absolute top-full right-0 z-40 mt-2 max-h-80 w-72 overflow-auto rounded-xl border border-line bg-surface-2 p-1.5 shadow-xl" aria-label="Matching components">
                            {move || {
                                let q = query.get().trim().to_lowercase();
                                let found = diagram.with(|d| d.nodes.iter().filter(|n| n.label.to_lowercase().contains(&q) || n.id.to_lowercase().contains(&q) || n.tags.iter().any(|t| t.to_lowercase().contains(&q)) || n.body_md.as_ref().is_some_and(|b| b.to_lowercase().contains(&q))).cloned().collect::<Vec<_>>());
                                if found.is_empty() { view! { <p class="p-3 text-xs text-muted">"No matching components"</p> }.into_any() }
                                else { found.into_iter().map(move |node| view! { <button type="button" class="nav-item" on:click=move |_| { query.set(String::new()); selected.set(Some(node.id.clone())); focus_node.set(Some(node.id.clone())); inspecting.set(true); }><Icon name=super::graph::node_card::kind_icon(&node.kind) size=15 /><span class="truncate">{node.label.clone()}</span></button> }).collect_view().into_any() }
                            }}
                        </div>
                    </Show>
                </search>
                <button type="button" class="icon-btn" aria-label="Undo" title="Undo (⌘Z)" on:click=move |_| undo()><Icon name="undo-2" size=16 /></button>
                <button type="button" class="icon-btn" aria-label="Redo" title="Redo (⌘⇧Z)" on:click=move |_| step(false)><Icon name="undo-2" size=16 class="-scale-x-100" /></button>
                <button type="button" class="btn bg-transparent px-3 py-2" on:click=move |_| agent_open.set(true)><Icon name="sparkles" size=15 /> "Ask agent"</button>
                <button type="button" class="icon-btn" aria-label="Delete diagram" title="Delete diagram" on:click=move |_| confirm_delete.set(true)><Icon name="trash-2" size=16 /></button>
            </header>
            <div class="relative flex min-h-0 flex-1">
                <Show when=move || !load_error.get().is_empty()>
                    <div class="m-auto max-w-[460px] p-6 text-center text-xs leading-relaxed text-danger-text">{move || load_error.get()}</div>
                </Show>
                <Show when=move || draft.with(Option::is_some)>
                    <DiagramCanvas
                        diagram=diagram
                        selected=selected
                        focus_node=focus_node
                        editing=editing
                        query=query
                        comments=comments
                        on_edit=on_edit
                        on_details=Callback::new(move |id: String| { selected.set(Some(id.clone())); focus_node.set(Some(id)); inspecting.set(true); })
                        panel=Signal::derive(move || inspecting.get())
                    />
                </Show>
                {move || {
                    if !inspecting.get() {
                        return None;
                    }
                    let id = selected.get()?;
                    let current = draft.get()?;
                    let node = current.nodes.iter().find(|n| n.id == id)?.clone();
                    Some(view! {
                        <NodeInspector
                            slug=inspector_slug.clone()
                            node=node
                            diagram=current
                            on_change=on_node
                            on_edges=on_edges
                            on_delete=on_delete
                            on_navigate=Callback::new(move |id: String| { selected.set(Some(id.clone())); focus_node.set(Some(id)); })
                            on_explain=Callback::new(move |node: DiagramNode| {
                                let refs = node.code_refs.iter().map(|r| r.reference()).collect::<Vec<_>>().join(", ");
                                ask_agent(workspace, agent_open, format!("Explain the {} component ‘{}’ in this diagram. Read the relevant code, describe its role and connections, and cite the evidence. Code references: {}", node.kind, node.label, if refs.is_empty() { "find the relevant code" } else { &refs }));
                            })
                            on_close=Callback::new(move |_: ()| inspecting.set(false))
                        />
                    })
                }}
            </div>
            <Show when=move || confirm_delete.get()>
                <Modal title="Delete this diagram?" on_close=Callback::new(move |_: ()| confirm_delete.set(false))>
                    <p class="text-[13px] leading-[1.9] text-ink">"The file is removed from the repository’s .schematlas folder. Git keeps its history if it was committed."</p>
                    <div class="modal-footer">
                        <button type="button" class="btn" on:click=move |_| confirm_delete.set(false)>"Cancel"</button>
                        <button type="button" class="btn btn-danger" on:click=move |_| delete.run(())>"Delete"</button>
                    </div>
                </Modal>
            </Show>
        </div>
    }
}

/// `path:line` mentions in a doc, in order of appearance, so readers can jump to the code.
fn code_mentions(markdown: &str) -> Vec<String> {
    let mut found = vec![];
    for (i, part) in markdown.split('`').enumerate() {
        let candidate = part.trim();
        let looks_like_path = i % 2 == 1
            && !candidate.contains(' ')
            && candidate.contains('/')
            && candidate.len() < 200
            && candidate
                .split(['#', ':'])
                .next()
                .is_some_and(|p| p.contains('.'));
        if looks_like_path && !found.iter().any(|f| f == candidate) {
            found.push(candidate.to_string());
        }
    }
    for piece in markdown.split("](").skip(1) {
        if let Some(target) = piece.split(')').next() {
            let target = target.trim();
            if let Some(slug) = target.strip_prefix("schematlas:diagram/") {
                let link = format!("schematlas:diagram/{slug}");
                if !found.contains(&link) {
                    found.push(link);
                }
            } else if !target.contains("://")
                && !target.starts_with('#')
                && !target.starts_with("mailto:")
                && target.contains('.')
                && !found.iter().any(|f| f == target)
            {
                found.push(target.to_string());
            }
        }
    }
    found
}

#[component]
pub fn DocView(slug: String) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let doc = RwSignal::new(None::<DocFile>);
    let editing = RwSignal::new(false);
    let text = RwSignal::new(String::new());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let project_id = workspace.project_id.get_untracked().unwrap_or_default();
    let load = {
        let (project_id, slug) = (project_id.clone(), slug.clone());
        move || {
            let (project_id, slug) = (project_id.clone(), slug.clone());
            spawn_local(async move {
                match api::get_doc(&project_id, &slug).await {
                    Ok(d) => {
                        text.set(d.markdown.clone());
                        doc.set(Some(d));
                    }
                    Err(e) => error.set(e),
                }
            });
        }
    };
    load();
    let watch_slug = slug.clone();
    let watch_load = load.clone();
    Effect::new(move |_| {
        let disk = workspace.repo.with(|r| {
            r.docs
                .iter()
                .find(|d| d.slug == watch_slug)
                .map(|d| d.hash.clone())
        });
        let ours = doc.with_untracked(|d| d.as_ref().map(|d| d.hash.clone()));
        if let (Some(disk), Some(ours)) = (disk, ours) {
            if disk != ours && !editing.get_untracked() {
                watch_load();
            }
        }
    });
    let save = {
        let (project_id, slug) = (project_id.clone(), slug.clone());
        move |_| {
            let (project_id, slug) = (project_id.clone(), slug.clone());
            let base = doc.with_untracked(|d| d.as_ref().map(|d| d.hash.clone()));
            spawn_local(async move {
                busy.set(true);
                match api::save_doc(&project_id, &slug, &text.get_untracked(), base.as_deref()).await {
                    Ok(saved) => {
                        doc.set(Some(saved));
                        editing.set(false);
                        error.set(String::new());
                        workspace.refresh_repo().await;
                        announce(workspace, "diagram_changed", json!({"diagram": format!("doc:{slug}"), "summary": "edited a doc"}));
                    }
                    Err(e) if conflict(&e) => error.set("This doc changed on disk while you were editing. Copy your text, then reload.".into()),
                    Err(e) => error.set(e),
                }
                busy.set(false);
            });
        }
    };
    let open = move |target: String| {
        if let Some(slug) = target.strip_prefix("schematlas:diagram/") {
            workspace.open_item(AtlasItem::Diagram(slug.to_string()));
            return;
        }
        let project_id = project_id.clone();
        spawn_local(async move {
            let reference = target.replace("#L", ":").replacen("-L", "-", 1);
            if let Err(e) = api::open_code_ref(&project_id, &reference).await {
                workspace.fail(e);
            }
        });
    };
    let delete_slug = slug.clone();
    let delete = move |_| {
        let Some((project_id, hash)) = workspace
            .project_id
            .get_untracked()
            .zip(doc.with_untracked(|d| d.as_ref().map(|d| d.hash.clone())))
        else {
            return;
        };
        let slug = delete_slug.clone();
        spawn_local(async move {
            match api::delete_doc(&project_id, &slug, &hash).await {
                Ok(()) => {
                    workspace.item.set(None);
                    workspace.refresh_repo().await;
                }
                Err(e) => workspace.fail(e),
            }
        });
    };
    let reload = load.clone();
    view! {
        <div class="flex h-full min-h-0 flex-col">
            <header class="flex min-h-[58px] shrink-0 items-center gap-3 border-b border-line bg-surface px-[18px]">
                <Icon name="file-text" size=17 class="text-sage" />
                <div class="flex min-w-0 flex-1 flex-col">
                    <h1 class="truncate text-sm font-semibold text-ink">{move || doc.with(|d| d.as_ref().map(|d| d.title.clone()).unwrap_or_default())}</h1>
                    <span class="font-mono text-[10px] text-faint">{format!(".schematlas/docs/{slug}.md")}</span>
                </div>
                {move || if editing.get() {
                    let save = save.clone();
                    let reload = reload.clone();
                    view! {
                        <button type="button" class="btn" on:click=move |_| { editing.set(false); reload(); }>"Cancel"</button>
                        <button type="button" class="btn btn-primary" disabled=move || busy.get() on:click=save>{move || if busy.get() { "Saving…" } else { "Save" }}</button>
                    }.into_any()
                } else {
                    view! {
                        <button type="button" class="btn" on:click=move |_| editing.set(true)><Icon name="pencil" size=14 /> " Edit"</button>
                        <button type="button" class="icon-btn" aria-label="Delete doc" on:click=delete.clone()><Icon name="trash-2" size=16 /></button>
                    }.into_any()
                }}
            </header>
            <FormError error=error />
            <div class="flex min-h-0 flex-1">
                <Show when=move || editing.get()>
                    <textarea class="field m-4 min-h-0 flex-1 resize-none font-mono text-xs leading-[1.7]" aria-label="Markdown" spellcheck="false" bind:value=text></textarea>
                </Show>
                <article class="markdown min-h-0 flex-1 overflow-auto px-10 py-8" inner_html=move || render_markdown(&text.get())></article>
                <aside class="w-[240px] shrink-0 overflow-auto border-l border-line p-4 max-[1100px]:hidden">
                    <div class="eyebrow mb-3">"REFERENCES"</div>
                    {move || {
                        let mentions = code_mentions(&text.get());
                        if mentions.is_empty() {
                            view! { <p class="form-hint">"Mention code as `src/file.rs:12` or link a diagram with (schematlas:diagram/name) to list it here."</p> }.into_any()
                        } else {
                            let open = open.clone();
                            view! {
                                <ul class="flex flex-col gap-1.5">
                                    {mentions.into_iter().map(|m| {
                                        let target = m.clone();
                                        let open = open.clone();
                                        let diagram = m.starts_with("schematlas:");
                                        view! {
                                            <li>
                                                <button type="button" class="flex w-full items-center gap-1.5 truncate rounded px-1.5 py-1 text-left font-mono text-[10px] text-accent-text hover:bg-surface-3" on:click=move |_| open(target.clone())>
                                                    {if diagram { view! { <Icon name="workflow" size=11 /> } } else { view! { <Icon name="code" size=11 /> } }}
                                                    <span class="truncate">{m.trim_start_matches("schematlas:diagram/").to_string()}</span>
                                                </button>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_any()
                        }
                    }}
                </aside>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_and_mentions() {
        assert_eq!(slug_of("Checkout flow (v2)"), "checkout-flow-v2");
        assert_eq!(unused("map".into(), |s| s == "map"), "map-2");
        let mentions = code_mentions("See `src/api/mod.rs:12-30`, `cargo test`, [map](schematlas:diagram/arch) and [x](https://e.com).");
        assert_eq!(
            mentions,
            ["src/api/mod.rs:12-30", "schematlas:diagram/arch"]
        );
    }
}
