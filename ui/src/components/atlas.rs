//! The repository atlas: the overview of `.schematlas/`, the diagram editor with its inspector,
//! Markdown docs, and the dialogs that create them. Files in the repository are the source of
//! truth; every save is compare-and-swap against the hash last read.
use super::diagram_preview::DiagramPreview;
use super::dialogs::{FormError, Modal};
use super::graph::diagram_canvas::DiagramCanvas;
use super::icons::Icon;
use super::team::{announce, comment_counts, open_comments, Avatar};
use crate::api;
use crate::diagram_layout::{plan, Edit};
use crate::markdown::{render_markdown, Segment};
use crate::state::{AtlasItem, Workspace, LOCAL_CHANGES_KEY as LOCAL_KEY};
use crate::types::{
    Annotation, CodeRef, Diagram, DiagramEdge, DiagramEdit, DiagramFile, DiagramNode, DocFile,
    Editable, Frame, Freshness, NodeChange, Snippet, Stale,
};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Create {
    Diagram,
    Doc,
    Mermaid,
}

fn conflict(error: &str) -> bool {
    error.contains("changed since")
}

/// A new, empty Mermaid flowchart titled `title`.
fn new_mermaid(title: &str) -> String {
    let title = title.replace(['\n', '\r'], " ").replace('"', "'");
    format!("---\ntitle: \"{}\"\n---\nflowchart LR\n", title.trim())
}

/// The icon for a diagram file's format.
pub fn format_icon(format: &str) -> &'static str {
    match format {
        "dbml" => "database",
        "drawio" => "layers-3",
        "markdown" => "file-text",
        _ => "workflow",
    }
}
/// Forget what was open, unless another view has taken over meanwhile.
fn clear_focus(workspace: Workspace, path: &str) {
    if workspace
        .focus
        .with_untracked(|f| f.as_ref().is_some_and(|f| f.path == path))
    {
        workspace.focus.set(None);
    }
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

const SMALL: &str = "btn min-h-7 bg-transparent px-2.5 py-1 text-[11px]";

/// A project's home: the prompt, the diagrams whose cited code changed, and past conversations.
/// Everything else lives in the explorer.
#[component]
pub fn AtlasOverview(
    agent_open: RwSignal<bool>,
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
    // A suggestion replaces the draft in the home composer, whose send starts a new conversation;
    // the side panel would continue the last one.
    let prepare = move |text: String| {
        workspace.agent_draft.set(text);
        agent_open.set(false);
    };
    let root = move || workspace.repo.with(|r| r.root.clone());
    // Diagrams with flagged nodes, most flagged first, titled as the repository lists them.
    let changed = Memo::new(move |_| {
        let mut rows: Vec<(String, String, Stale)> = workspace.stale.with(|f| {
            f.diagrams
                .iter()
                .filter(|(_, s)| s.total() > 0)
                .map(|(id, s)| {
                    let title = workspace.repo.with(|r| {
                        r.diagrams
                            .iter()
                            .find(|d| &d.id == id)
                            .map(|d| d.title.clone())
                            .unwrap_or_else(|| id.clone())
                    });
                    (id.clone(), title, *s)
                })
                .collect()
        });
        rows.sort_by_key(|(_, _, s)| std::cmp::Reverse(s.total()));
        rows
    });
    let branch = move || workspace.stale.with(|f| f.branch.clone());
    view! {
        <header class="flex h-16 shrink-0 items-center gap-4 border-b border-line-soft px-6">
            <div class="min-w-0">
                <h1 class="truncate text-[15px] font-medium tracking-tight">{move || workspace.project.with(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default())}</h1>
                <p class="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11px] text-muted">
                    <Icon name="folder-git" size=12 />
                    {move || match root() {
                        _ if !desktop => view! { <span>"Example repository · Browser preview"</span> }.into_any(),
                        Some(path) => view! { <span class="truncate font-mono" title=path.clone()>{path.clone()}</span> }.into_any(),
                        None => view! { <span>"No repository connected"</span> }.into_any(),
                    }}
                </p>
            </div>
            <div class="ml-auto flex shrink-0 items-center gap-2">
                <Show when=move || desktop && has_repo()>
                    <button type="button" class="btn bg-transparent px-3 py-2 text-xs" disabled=move || busy.get() on:click=choose_repo><Icon name="folder-git" size=14 /> "Change repository"</button>
                </Show>
            </div>
        </header>
        <div class="min-h-0 flex-1 overflow-auto">
            <Show when=has_repo fallback=move || view! {
                <section class="mx-auto mt-[12vh] w-full max-w-[520px] rounded-2xl border border-line bg-surface p-8 text-center">
                    <div class="mx-auto mb-4 flex size-12 items-center justify-center rounded-2xl bg-surface-3 text-sage"><Icon name="folder-git" size=22 /></div>
                    <h2 class="text-base font-medium">"Connect this project to a repository"</h2>
                    <p class="mt-2 text-xs leading-6 text-muted">"Its diagrams and docs are read from the repository, and your agent reads the code to propose more."</p>
                    <Show when=move || desktop fallback=|| view! { <p class="mt-5 text-xs text-muted">"Browser preview · open the desktop app to choose a repository."</p> }>
                        <button type="button" class="btn btn-primary mt-6" disabled=move || busy.get() on:click=choose_repo><Icon name="folder-open" size=15 /> "Choose repository"</button>
                    </Show>
                    <div class="mt-6 flex justify-center gap-2 border-t border-line-soft pt-5">
                        <button type="button" class=SMALL on:click=move |_| on_connect.run(())><Icon name="database" size=13 /> "Connect database"</button>
                        <button type="button" class=SMALL on:click=move |_| on_import.run(())><Icon name="braces" size=13 /> "Import OpenAPI"</button>
                    </div>
                </section>
            }>
                <div class="mx-auto flex w-full max-w-[760px] flex-col gap-9 px-6 pt-[8vh] pb-10">
                    <section aria-label="Ask your agent">
                        <Show when=move || !agent_open.get() fallback=|| view! {
                            <p class="rounded-2xl border border-dashed border-line px-4 py-3 text-xs text-muted">"Your agent is open on the right."</p>
                        }>
                            <div class="[&_textarea]:min-h-[44px]">
                                {move || workspace.project.with(|p| p.as_ref().map(|p| view! {
                                    <super::agent::AgentPanel project_id=p.id.clone() project_name=p.name.clone() inline=true on_start=Callback::new(move |()| agent_open.set(true)) on_close=Callback::new(|()| {}) />
                                })).map(IntoView::into_view)}
                            </div>
                        </Show>
                        <div class="mt-3 flex flex-wrap justify-center gap-2">
                            {[
                                ("workflow", "Map the architecture", "Map this repository: propose a focused architecture diagram of the main components and their connections, with code references and a short overview document."),
                                ("spline", "Explain a flow", "Help me choose a critical flow in this repository, then explain it step by step with a diagram and code references."),
                                ("file-text", "Generate the wiki", "Generate this repository's wiki: follow the repository wiki workflow — survey it with repo_outline, propose the plan in toc.yaml and wait for my approval, then write the pages and their diagrams with code references.")
                            ].into_iter().map(move |(icon, label, request)| view! {
                                <button type="button" class="flex items-center gap-2 rounded-full border border-line px-3 py-1.5 text-[11px] text-soft hover:bg-surface-3 hover:text-ink" on:click=move |_| prepare(request.into())><Icon name=icon size=13 />{label}</button>
                            }).collect_view()}
                        </div>
                    </section>
                    <Show when=move || !changed.with(Vec::is_empty)>
                        <section aria-label="Code changes">
                            <div class="mb-4 flex items-center justify-between">
                                <h2 class="text-sm font-medium">"Code changes"</h2>
                                <span class="text-[11px] text-muted">{move || format!("Diagrams citing code that changed on {}", branch())}</span>
                            </div>
                            <ul class="overflow-hidden rounded-xl border border-line bg-surface">
                                <For each=move || changed.get() key=|(id, _, s)| (id.clone(), *s) children=move |(id, title, stale)| {
                                    let open_id = id.clone();
                                    view! { <li class="border-b border-line-soft last:border-b-0">
                                        <button type="button" class="flex min-h-14 w-full items-center gap-3 px-4 py-2.5 text-left hover:bg-surface-3" on:click=move |_| workspace.open_item(AtlasItem::Diagram(open_id.clone()))>
                                            <Icon name="workflow" size=16 class="shrink-0 text-muted" />
                                            <span class="min-w-0 flex-1">
                                                <span class="block truncate text-xs font-medium">{title}</span>
                                                <span class="mt-1 block truncate font-mono text-[10px] text-faint">{id}</span>
                                            </span>
                                            <super::explorer::StaleBadge stale=stale />
                                            <Icon name="arrow-up-right" size=14 class="text-muted" />
                                        </button>
                                    </li> }
                                } />
                            </ul>
                        </section>
                    </Show>
                    <Show when=move || desktop>
                        {move || workspace.project.with(|p| p.as_ref().map(|p| view! { <super::agent::ConversationList project_id=p.id.clone() agent_open=agent_open /> }))}
                    </Show>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn CreateDialog(which: Create, #[prop(into)] on_close: Callback<()>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let destination = RwSignal::new(String::new());
    let toc_base = RwSignal::new(workspace.toc.with_untracked(|s| s.hash.clone()));
    let title = RwSignal::new(String::new());
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
            // Initialize once before a non-document creator runs, so migration cannot absorb
            // the new file before its chosen TOC location is registered.
            if which != Create::Doc && toc_base.get_untracked().is_none() {
                match api::edit_toc(&project_id, None, None).await {
                    Ok(snapshot) => {
                        toc_base.set(snapshot.hash.clone());
                        workspace.toc.set(snapshot);
                    }
                    Err(e) => {
                        error.set(e);
                        busy.set(false);
                        return;
                    }
                }
            }
            let result = match which {
                Create::Diagram => {
                    api::create_diagram(&project_id, None, &name, &new_mermaid(&name))
                        .await
                        .map(|f| AtlasItem::Diagram(f.id))
                }
                Create::Mermaid => {
                    api::create_diagram(&project_id, None, &name, &mermaid.get_untracked())
                        .await
                        .map(|f| AtlasItem::Diagram(f.id))
                }
                Create::Doc => {
                    let folder = destination.get_untracked();
                    let base = toc_base.get_untracked();
                    api::create_toc_doc(
                        &project_id,
                        &name,
                        (!folder.is_empty()).then_some(folder.as_str()),
                        base.as_deref(),
                    )
                    .await
                    .map(|created| {
                        workspace.toc.set(created.toc);
                        AtlasItem::Doc(created.doc.slug)
                    })
                }
            };
            match result {
                Ok(item) => {
                    if let AtlasItem::Diagram(id) = &item {
                        let folder = destination.get_untracked();
                        let mut entry = schematlas_ui::toc::Entry::file(name.clone(), id.clone());
                        entry.kind = Some("diagram".into());
                        let base = toc_base.get_untracked();
                        match api::edit_toc(&project_id, Some(&schematlas_ui::toc::Edit::Add { entry, parent: (!folder.is_empty()).then_some(folder) }), base.as_deref()).await {
                            Ok(snapshot) => workspace.toc.set(snapshot),
                            Err(e) => workspace.fail(format!("The diagram was created but could not be added to toc.yaml. Find it in the explorer under In the repository. {e}")),
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
            "Saved as .schematlas/diagrams/<name>.mmd, a Mermaid file GitHub and editors display too.",
        ),
        Create::Doc => (
            "New doc",
            "A Markdown file in your documentation folder, included in toc.yaml.",
        ),
        Create::Mermaid => (
            "Paste Mermaid",
            "A flowchart, class, ER or state diagram, saved as is in .schematlas/diagrams/.",
        ),
    };
    view! {
        <Modal title=heading subtitle=subtitle on_close=on_close busy=busy>
            <form on:submit=submit>
                <label class="form-label" for="atlas-title">"Title " <span class="text-accent">"*"</span></label>
                <input id="atlas-title" class="field mb-[21px]" bind:value=title placeholder="e.g. Checkout flow" required maxlength="120" />
                <super::explorer::FolderSelect value=destination />
                {(which == Create::Mermaid).then(|| view! {
                    <label class="form-label" for="atlas-mermaid">"Mermaid"</label>
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
                let reference = code_ref.to_string();
                let peek_ref = reference.clone();
                let open_ref = reference.clone();
                view! {
                    <li class="rounded-md border border-line bg-surface-3">
                        <div class="flex items-center gap-1 px-2 py-1.5">
                            <button type="button" class="min-w-0 flex-1 truncate text-left font-mono text-[11px] text-accent-text hover:underline" title="Show the code" on:click=move |_| peek(index, peek_ref.clone())>
                                {reference.clone()}
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
    diagram_id: String,
    node: DiagramNode,
    diagram: Arc<Diagram>,
    /// Only annotations can change: the structure is written in the file.
    locked: bool,
    #[prop(into)] on_edit: Callback<Edit>,
    #[prop(into)] on_explain: Callback<DiagramNode>,
    #[prop(into)] on_navigate: Callback<String>,
    #[prop(into)] on_close: Callback<()>,
    /// Cited code that changed since the node was checked, and against which branch.
    change: Option<NodeChange>,
    branch: Option<String>,
    #[prop(into)] on_review: Callback<String>,
    #[prop(into)] on_update: Callback<(DiagramNode, NodeChange)>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let id = node.id.clone();
    let changed_view = change.clone().map(|change| {
        let broken = change.status == "broken";
        let local = change.status == "local";
        let heading = if broken {
            "Code reference broken".to_string()
        } else if local {
            "Changed in your working copy".to_string()
        } else {
            format!("Code changed on {}", branch.clone().unwrap_or_else(|| "the reference branch".into()))
        };
        let (review_id, update_node, update_change) = (id.clone(), node.clone(), change.clone());
        view! {
            <section class="rounded-xl border p-3" class=("border-danger-line", broken) class=("border-warning", !broken) aria-label="Code changes">
                <div class="mb-2 flex items-center gap-1.5 text-xs font-medium" class=("text-danger-text", broken) class=("text-warning", !broken)>
                    <Icon name="git-branch" size=13 /> {heading}
                </div>
                <p class="mb-2 text-[11px] leading-5 text-muted">{format!("Since {} the code this node cites was changed. Check that it still says what the code does.", change.baseline.chars().take(8).collect::<String>())}</p>
                <ul class="flex flex-col gap-2">
                    {change.refs.into_iter().map(|r| view! {
                        <li class="rounded-md border border-line bg-surface-3">
                            <div class="flex items-center gap-2 px-2 py-1.5 font-mono text-[10px]">
                                <span class="min-w-0 flex-1 truncate text-ink" title=r.reference.clone()>{r.reference.clone()}</span>
                                {r.now.clone().filter(|n| *n != r.reference).map(|n| view! { <span class="truncate text-muted" title="Where the code is now">{format!("→ {n}")}</span> })}
                            </div>
                            {r.reason.clone().map(|reason| view! { <p class="border-t border-line px-2 py-1.5 text-[11px] text-danger-text">{reason}</p> })}
                            {(!r.commits.is_empty()).then(|| view! {
                                <ul class="border-t border-line px-2 py-1.5 text-[10px] text-soft">
                                    {r.commits.iter().map(|c| view! { <li class="truncate font-mono" title=c.clone()>{c.clone()}</li> }).collect_view()}
                                </ul>
                            })}
                            {r.patch.clone().filter(|p| !p.is_empty()).map(|patch| view! {
                                <details class="border-t border-line">
                                    <summary class="cursor-pointer px-2 py-1 text-[10px] text-muted">"What changed"</summary>
                                    <pre class="max-h-56 overflow-auto px-2 pb-2 font-mono text-[10px] leading-[1.6]">
                                        {patch.lines().map(|line| {
                                            let class = if line.starts_with('+') { "text-accent" } else if line.starts_with('-') { "text-danger-text" } else { "text-muted" };
                                            view! { <div class=class>{line.to_string()}</div> }
                                        }).collect_view()}
                                    </pre>
                                </details>
                            })}
                        </li>
                    }).collect_view()}
                </ul>
                <div class="mt-3 flex flex-wrap gap-2">
                    <button type="button" class="btn min-h-8 px-3 py-1.5 text-[11px]" title="The node still holds: follow the code and record this check" on:click=move |_| on_review.run(review_id.clone())><Icon name="check" size=13 /> "Reviewed"</button>
                    <button type="button" class="btn min-h-8 bg-transparent px-3 py-1.5 text-[11px]" on:click=move |_| on_update.run((update_node.clone(), update_change.clone()))><Icon name="sparkles" size=13 /> "Ask agent to update"</button>
                </div>
            </section>
        }
    });
    let annotation = StoredValue::new(node.annotation.clone());
    let annotate = {
        let id = id.clone();
        move |change: &dyn Fn(&mut Annotation)| {
            let mut next = annotation.get_value();
            change(&mut next);
            if next != annotation.get_value() {
                annotation.set_value(next.clone());
                on_edit.run(Edit::Annotate {
                    id: id.clone(),
                    annotation: next,
                });
            }
        }
    };
    let new_ref = RwSignal::new(String::new());
    let comment = RwSignal::new(String::new());
    let labels: HashMap<String, String> = diagram
        .nodes
        .iter()
        .map(|n| (n.id.clone(), n.label.clone()))
        .chain(
            diagram
                .groups
                .iter()
                .map(|g| (g.id.clone(), g.title.clone())),
        )
        .collect();
    let connections: Vec<(usize, DiagramEdge)> = diagram
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.from == id || e.to == id)
        .map(|(i, e)| (i, e.clone()))
        .collect();
    let thread_diagram = diagram_id.clone();
    let thread_id = id.clone();
    let thread = Memo::new(move |_| {
        workspace.sync.with(|s| {
            s.as_ref()
                .map(|s| {
                    open_comments(&s.events, &thread_diagram)
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
    let post_diagram = diagram_id.clone();
    let post_id = id.clone();
    let post = move || {
        let text = comment.get_untracked().trim().to_string();
        if text.is_empty() {
            return;
        }
        announce(
            workspace,
            "comment",
            json!({"diagram": post_diagram.clone(), "node": post_id.clone(), "text": text}),
        );
        comment.set(String::new());
    };
    let delete_id = id.clone();
    let rename_id = id.clone();
    let explain_node = node.clone();
    let note = node
        .annotation
        .note
        .clone()
        .filter(|b| !b.trim().is_empty());
    let no_description = note.is_none();
    let (kind_edit, note_edit, tags_edit, refs_remove, refs_add) = (
        annotate.clone(),
        annotate.clone(),
        annotate.clone(),
        annotate.clone(),
        annotate,
    );
    view! {
        <aside class="absolute top-0 right-0 bottom-0 z-20 flex w-[340px] animate-rise-in flex-col border-l border-line bg-surface-2 shadow-[-12px_0_35px_#0005]" aria-label="Node details" on:keydown=|ev| ev.stop_propagation()>
            <header class="flex items-center justify-between border-b border-line px-4 py-3.5">
                <span class="text-xs font-medium text-muted">"Component details"</span>
                <button type="button" class="icon-btn" aria-label="Close node details" on:click=move |_| on_close.run(())><Icon name="x" size=16 /></button>
            </header>
            <div class="flex min-h-0 flex-1 flex-col gap-5 overflow-auto p-5">
                {changed_view}
                <div>
                    <span class="text-[11px] text-muted">{node.kind().to_owned()}</span>
                    <h2 class="mt-1 text-lg font-medium tracking-tight"><super::graph::node_card::LabelText label=node.label.clone() /></h2>
                    {note.map(|body| view! { <div class="markdown mt-4" inner_html=render_markdown(&body)></div> })}
                    <Show when=move || no_description><p class="mt-3 text-xs leading-6 text-muted">"No explanation yet. Ask your agent to describe this component and link it to the code."</p></Show>
                    {(!node.details.is_empty()).then(|| view! {
                        <ul class="mt-4 rounded-md border border-line bg-surface-3 py-1 font-mono text-[11px] text-soft" aria-label="Members">
                            {node.details.iter().map(|line| view! { <li class="px-2.5 py-0.5 [overflow-wrap:anywhere]">{line.clone()}</li> }).collect_view()}
                        </ul>
                    })}
                </div>
                <details class="rounded-xl border border-line p-3">
                    <summary class="cursor-pointer text-xs font-medium text-soft">"Edit details"</summary>
                    <div class="mt-4 flex flex-col gap-4">
                <div>
                    <label class="form-label" for="node-label">"Label"</label>
                    <input id="node-label" class="field" prop:value=node.label.clone() maxlength="200" disabled=locked
                        on:change=move |ev| { let v = event_target_value(&ev); if !v.trim().is_empty() { on_edit.run(Edit::Rename { id: rename_id.clone(), label: v }); } } />
                </div>
                <div>
                    <label class="form-label" for="node-kind">"Kind"</label>
                    <input id="node-kind" class="field" list="node-kinds" prop:value=node.kind().to_owned() maxlength="40"
                        on:change=move |ev| { let v = event_target_value(&ev).trim().to_lowercase().replace(' ', "-"); if !v.is_empty() { kind_edit(&|a| a.kind = Some(v.clone())); } } />
                    <datalist id="node-kinds">{NODE_KINDS.iter().map(|k| view! { <option value=*k></option> }).collect_view()}</datalist>
                </div>
                <div>
                    <label class="form-label" for="node-body">"Description"</label>
                    <textarea id="node-body" class="field min-h-[110px] resize-y" prop:value=node.annotation.note.clone().unwrap_or_default()
                        on:change=move |ev| { let v = event_target_value(&ev); note_edit(&|a| a.note = Some(v.trim().to_string()).filter(|b| !b.is_empty())); }></textarea>
                </div>
                <div>
                    <label class="form-label" for="node-tags">"Tags " <small class="font-normal text-muted">"comma separated"</small></label>
                    <input id="node-tags" class="field" prop:value=node.annotation.tags.join(", ")
                        on:change=move |ev| { let v = event_target_value(&ev); tags_edit(&|a| a.tags = v.split(',').map(|t| t.trim().replace(' ', "-")).filter(|t| !t.is_empty()).collect()); } />
                </div>
                    <p class="form-hint">"Saved in the diagram file as a comment other tools ignore."</p>
                    </div>
                </details>
                <div>
                    <div class="form-label">"Source evidence"</div>
                    <CodeRefs refs=node.annotation.code.clone() on_remove=Callback::new(move |i: usize| refs_remove(&|a| { if i < a.code.len() { a.code.remove(i); } })) />
                    <form class="mt-2 flex gap-1.5" on:submit=move |ev: leptos::ev::SubmitEvent| {
                        ev.prevent_default();
                        let v = new_ref.get_untracked().trim().to_string();
                        if !v.is_empty() {
                            refs_add(&|a| a.code.push(CodeRef::parse(&v)));
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
                                    let outgoing = edge.from == id;
                                    let other_id = if outgoing { edge.to.clone() } else { edge.from.clone() };
                                    let other = labels.get(&other_id).cloned().unwrap_or_else(|| other_id.clone());
                                    view! {
                                        <li class="flex items-center gap-1.5 text-[11px]">
                                            <span class="w-4 text-faint">{if outgoing { "→" } else { "←" }}</span>
                                            <button type="button" class="w-24 truncate text-left text-ink hover:underline" title=other.clone() aria-label=format!("Go to {other}") on:click=move |_| on_navigate.run(other_id.clone())>{other.clone()}</button>
                                            <input class="field min-w-0 flex-1 py-1" placeholder="label" prop:value=edge.label.clone().unwrap_or_default() disabled=locked
                                                on:change=move |ev| on_edit.run(Edit::EdgeLabel { index, label: event_target_value(&ev) }) />
                                            <Show when=move || !locked>
                                                <button type="button" class="icon-btn size-6" aria-label="Remove connection" on:click=move |_| on_edit.run(Edit::Delete { nodes: vec![], edges: vec![index] })><Icon name="x" size=13 /></button>
                                            </Show>
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
                <Show when=move || !locked>
                    {let delete_id = delete_id.clone(); view! {
                        <button type="button" class="btn btn-ghost-danger w-full justify-start" on:click=move |_| on_edit.run(Edit::Delete { nodes: vec![delete_id.clone()], edges: vec![] })><Icon name="trash-2" size=14 /> " Delete node"</button>
                    }}
                </Show>
            </footer>
        </aside>
    }
}

/// The file as it was before a change, for undo: its text and the positions kept beside it.
#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    text: String,
    layout: std::collections::BTreeMap<String, Frame>,
}
impl Snapshot {
    fn of(file: &DiagramFile) -> Self {
        Snapshot {
            text: file.text.clone(),
            layout: file.diagram.layout.clone(),
        }
    }
}
/// Moves that put nodes back where `snapshot` had them, for formats that keep positions beside
/// the file (draw.io positions are in the text an undo restores).
fn moves_back(snapshot: &Snapshot, file: &DiagramFile) -> Vec<DiagramEdit> {
    if file.diagram.kind == schematlas_parser::model::DRAWIO {
        return vec![];
    }
    let mut ids: Vec<&String> = snapshot.layout.keys().collect();
    ids.sort();
    ids.into_iter()
        .filter(|id| file.diagram.nodes.iter().any(|n| &n.id == *id))
        .filter_map(|id| {
            let before = snapshot.layout[id];
            let now = file.diagram.layout.get(id);
            (now.map(|r| (r.x, r.y)) != Some((before.x, before.y))).then(|| DiagramEdit::Move {
                id: id.clone(),
                x: before.x,
                y: before.y,
            })
        })
        .collect()
}
/// The title people see: Schematlas's own, the file's, or the backend's guess from the path.
fn shown_title(diagram: &Diagram, file: Option<&DiagramFile>) -> String {
    diagram
        .annotation
        .title
        .clone()
        .or_else(|| diagram.title.clone())
        .or_else(|| file.map(|f| f.title.clone()))
        .unwrap_or_default()
}

#[component]
pub fn DiagramView(id: String, agent_open: RwSignal<bool>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let file = RwSignal::new(None::<DiagramFile>);
    let draft = RwSignal::new(None::<Arc<Diagram>>);
    let load_error = RwSignal::new(String::new());
    let selected = RwSignal::new(None::<String>);
    let query = RwSignal::new(String::new());
    let focus_node = RwSignal::new(None::<String>);
    // A node an agent asked to show in this diagram: select it and bring it into view.
    {
        let id = id.clone();
        Effect::new(move |_| {
            let Some((diagram, node)) = workspace.reveal.get() else {
                return;
            };
            if diagram == id {
                workspace.reveal.set(None);
                selected.set(Some(node.clone()));
                focus_node.set(Some(node));
            }
        });
    }
    let history = StoredValue::new(Vec::<Snapshot>::new());
    let future = StoredValue::new(Vec::<Snapshot>::new());
    let editing = RwSignal::new(None::<String>);
    // The details panel opens on request and then follows the selection until closed.
    let inspecting = RwSignal::new(false);
    // Edits wait here while the previous batch is being written; each batch is one undo step.
    let queue = StoredValue::new(Vec::<DiagramEdit>::new());
    let saving = StoredValue::new(false);
    let announce_timer = StoredValue::new_local(None::<TimeoutHandle>);
    let confirm_delete = RwSignal::new(false);
    let project_id = workspace.project_id.get_untracked().unwrap_or_default();

    let editable =
        Memo::new(move |_| file.with(|f| f.as_ref().map(|f| f.editable).unwrap_or_default()));
    let locked = Signal::derive(move || editable.get() != Editable::Full);
    let load = {
        let (project_id, id) = (project_id.clone(), id.clone());
        move || {
            let (project_id, id) = (project_id.clone(), id.clone());
            spawn_local(async move {
                match api::get_diagram(&project_id, &id).await {
                    Ok(f) => {
                        queue.set_value(vec![]);
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
    // Show the latest file with whatever is still queued drawn on top.
    let settle = move |next: DiagramFile| {
        let pending = queue.get_value();
        let shown = pending.iter().try_fold(next.diagram.clone(), |d, edit| {
            crate::diagram_layout::apply_edit(&d, edit)
        });
        draft.set(Some(Arc::new(
            shown.unwrap_or_else(|_| next.diagram.clone()),
        )));
        file.set(Some(next));
    };
    let failed = {
        let load = load.clone();
        move |e: String| {
            queue.set_value(vec![]);
            if conflict(&e) {
                workspace.notice.set("This diagram changed on disk (an agent, an editor or git). Reloaded the latest version.".into());
            } else {
                workspace.fail(e);
            }
            load();
        }
    };
    let flush = {
        let (project_id, id, failed) = (project_id.clone(), id.clone(), failed.clone());
        StoredValue::new_local(move || {
            if saving.get_value() || queue.with_value(Vec::is_empty) {
                return;
            }
            let Some(current) = file.get_untracked() else {
                return;
            };
            let edits = queue.try_update_value(std::mem::take).unwrap_or_default();
            saving.set_value(true);
            let (project_id, id, failed) = (project_id.clone(), id.clone(), failed.clone());
            spawn_local(async move {
                let result = api::edit_diagram(&project_id, &id, &edits, &current.hash).await;
                saving.set_value(false);
                match result {
                    Ok(next) => {
                        history.update_value(|h| {
                            h.push(Snapshot::of(&current));
                            if h.len() > 100 {
                                h.remove(0);
                            }
                        });
                        future.update_value(Vec::clear);
                        settle(next);
                        workspace.refresh_repo().await;
                    }
                    Err(e) => failed(e),
                }
            });
        })
    };
    // A batch queued while another was being written goes out once that one returns.
    Effect::new(move |_| {
        file.track();
        flush.with_value(|f| f());
    });
    let note_id = id.clone();
    let change = Callback::new(
        move |(edits, next, content): (Vec<DiagramEdit>, Diagram, bool)| {
            draft.set(Some(Arc::new(next)));
            queue.update_value(|q| q.extend(edits));
            flush.with_value(|f| f());
            if content {
                let diagram_id = note_id.clone();
                announce_timer.update_value(|t| {
                    if let Some(t) = t.take() {
                        t.clear();
                    }
                });
                announce_timer.set_value(
                set_timeout_with_handle(
                    move || {
                        let title = draft
                            .with_untracked(|d| d.as_ref().map(|d| shown_title(d, None)))
                            .unwrap_or_default();
                        announce(
                            workspace,
                            "diagram_changed",
                            json!({"diagram": diagram_id, "summary": format!("edited “{title}”")}),
                        );
                    },
                    Duration::from_secs(8),
                )
                .ok(),
            );
            }
        },
    );
    // Undo and redo put back the file's earlier text, then the positions kept beside it.
    let step = {
        let (project_id, id, failed) = (project_id.clone(), id.clone(), failed.clone());
        move |back: bool| {
            if saving.get_value() || !queue.with_value(Vec::is_empty) {
                return;
            }
            let (from, to) = if back {
                (history, future)
            } else {
                (future, history)
            };
            let Some(current) = file.get_untracked() else {
                return;
            };
            let Some(snapshot) = from.try_update_value(|h| h.pop()).flatten() else {
                return;
            };
            to.update_value(|h| h.push(Snapshot::of(&current)));
            editing.set(None);
            saving.set_value(true);
            let (project_id, id, failed) = (project_id.clone(), id.clone(), failed.clone());
            spawn_local(async move {
                let mut result = Ok(current.clone());
                if snapshot.text != current.text {
                    result =
                        api::restore_diagram(&project_id, &id, &snapshot.text, &current.hash).await;
                }
                if let Ok(restored) = &result {
                    let moves = moves_back(&snapshot, restored);
                    if !moves.is_empty() {
                        result = api::edit_diagram(&project_id, &id, &moves, &restored.hash).await;
                    }
                }
                saving.set_value(false);
                match result {
                    Ok(next) => {
                        settle(next);
                        workspace.refresh_repo().await;
                    }
                    Err(e) => failed(e),
                }
            });
        }
    };
    let undo = {
        let step = step.clone();
        move || step(true)
    };
    let redo = {
        let step = step.clone();
        move || step(false)
    };
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
    let watch_id = id.clone();
    let watch_load = load.clone();
    Effect::new(move |_| {
        let disk = workspace.repo.with(|r| {
            r.diagrams
                .iter()
                .find(|d| d.id == watch_id)
                .map(|d| d.hash.clone())
        });
        let ours = file.with_untracked(|f| f.as_ref().map(|f| f.hash.clone()));
        let idle = !saving.get_value() && queue.with_value(Vec::is_empty);
        if let (Some(disk), Some(ours)) = (disk, ours) {
            if !disk.is_empty() && disk != ours && idle {
                watch_load();
            }
        }
    });
    // The agent is told which diagram is open, and which node is selected.
    let focus_id = id.clone();
    Effect::new(move |_| {
        let node = selected.get().and_then(|n| {
            draft.with(|d| {
                d.as_ref()?
                    .nodes
                    .iter()
                    .find(|x| x.id == n)
                    .map(|x| (x.id.clone(), crate::label::plain(&x.label)))
            })
        });
        workspace.focus.set(Some(crate::state::Focus {
            kind: "diagram",
            path: focus_id.clone(),
            node,
        }));
    });
    let mine = id.clone();
    on_cleanup(move || clear_focus(workspace, &mine));
    // Presence: teammates see which diagram you have open.
    announce(workspace, "editing", json!({"diagram": id.clone()}));
    on_cleanup(move || announce(workspace, "editing_done", json!({})));

    let diagram = Signal::derive(move || draft.get().unwrap_or_default());
    // Cited code that changed on the reference branch since each node was checked.
    let freshness = RwSignal::new(None::<Freshness>);
    let show_local = RwSignal::new(api::local_storage_get(LOCAL_KEY).as_deref() == Some("on"));
    let check_project = project_id.clone();
    let check_id = id.clone();
    let check = move || {
        if !api::desktop() {
            return;
        }
        let (project_id, id, local) = (
            check_project.clone(),
            check_id.clone(),
            show_local.get_untracked(),
        );
        spawn_local(async move {
            match api::diagram_freshness(&project_id, &id, local).await {
                Ok(found) => {
                    freshness.try_set(Some(found));
                }
                // Not a git repository, or git is missing: nothing to tell.
                Err(_) => {
                    freshness.try_set(None);
                }
            }
        });
    };
    Effect::new({
        let check = check.clone();
        move |_| {
            file.with(|f| f.as_ref().map(|f| f.hash.clone()));
            show_local.track();
            workspace.repo.track();
            check();
        }
    });
    let changes = Signal::derive(move || {
        freshness.with(|f| {
            f.as_ref()
                .map(|f| {
                    f.nodes
                        .iter()
                        .map(|(id, c)| (id.clone(), c.status.clone()))
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default()
        })
    });
    let review_project = project_id.clone();
    let review_id = id.clone();
    let review = {
        let failed = failed.clone();
        Callback::new(move |node: String| {
            let Some(current) = file.get_untracked() else {
                return;
            };
            if saving.get_value() || !queue.with_value(Vec::is_empty) {
                return;
            }
            saving.set_value(true);
            let (project_id, id, failed) =
                (review_project.clone(), review_id.clone(), failed.clone());
            spawn_local(async move {
                let result = api::review_node(&project_id, &id, &node, &current.hash).await;
                saving.set_value(false);
                match result {
                    Ok(next) => {
                        history.update_value(|h| h.push(Snapshot::of(&current)));
                        future.update_value(Vec::clear);
                        settle(next);
                        workspace.notice.set(
                            "Marked as reviewed: its code references follow the code again.".into(),
                        );
                        workspace.refresh_stale().await;
                    }
                    Err(e) => failed(e),
                }
            });
        })
    };
    // DBML is drawn like a connected database: the schema canvas, with the file's positions
    // and table groups.
    let schema_source = Signal::derive(move || {
        let graph = file.with(|f| {
            f.as_ref()
                .and_then(|f| f.schema.clone())
                .unwrap_or_default()
        });
        diagram.with(|d| {
            Arc::new(crate::types::Source {
                id: file.with(|f| f.as_ref().map(|f| f.id.clone()).unwrap_or_default()),
                name: shown_title(d, None),
                kind: "database".into(),
                graph,
                positions: d
                    .layout
                    .iter()
                    .map(|(id, r)| (id.clone(), crate::types::Position { x: r.x, y: r.y }))
                    .collect(),
                groups: Some(crate::diagram_layout::to_groups(d)),
                ..crate::types::Source::default()
            })
        })
    });
    let comment_id = id.clone();
    let comments = Signal::derive(move || {
        workspace
            .sync
            .with(|s| comment_counts(s.as_ref(), &comment_id))
    });
    let on_edit = Callback::new(move |edit: Edit| {
        let Some(current) = draft.get_untracked() else {
            return;
        };
        match editable.get_untracked() {
            Editable::None => {
                workspace
                    .notice
                    .set("This file is read-only here; edit the file it was exported from.".into());
                return;
            }
            Editable::Annotations if !edit.annotation_only() => {
                workspace.notice.set("This diagram’s structure is written in its file. Here you can describe, link and arrange it; edit the file (or ask your agent) to change the rest.".into());
                return;
            }
            _ => {}
        }
        let content = !matches!(edit, Edit::Layout(_));
        // While a diagram is laid out automatically, new cards are placed by the layout too;
        // only a deliberate drag (a Layout gesture) keeps an arrangement beside the file.
        let automatic = current.kind != schematlas_parser::model::DRAWIO
            && crate::diagram_layout::saved_positions(&current).is_empty();
        match plan(&current, edit) {
            Ok(mut planned) if !planned.edits.is_empty() => {
                if automatic && content {
                    planned
                        .edits
                        .retain(|e| !matches!(e, DiagramEdit::Move { .. }));
                    planned.diagram.layout.clear();
                }
                if !planned.edits.is_empty() {
                    change.run((planned.edits, planned.diagram, content));
                }
            }
            Ok(_) => {}
            Err(e) => workspace.notice.set(e),
        }
    });
    // Back to automatic layout: one undo step, like any other change.
    let auto_layout = {
        let (project_id, id, failed) = (project_id.clone(), id.clone(), failed.clone());
        Callback::new(move |_: ()| {
            if saving.get_value() || !queue.with_value(Vec::is_empty) {
                return;
            }
            let Some(current) = file.get_untracked() else {
                return;
            };
            saving.set_value(true);
            let (project_id, id, failed) = (project_id.clone(), id.clone(), failed.clone());
            spawn_local(async move {
                let result = api::reset_diagram_layout(&project_id, &id).await;
                saving.set_value(false);
                match result {
                    Ok(next) => {
                        history.update_value(|h| h.push(Snapshot::of(&current)));
                        future.update_value(Vec::clear);
                        settle(next);
                    }
                    Err(e) => failed(e),
                }
            });
        })
    };
    let rename = move |title: String| {
        let Some(current) = draft.get_untracked() else {
            return;
        };
        let title = title.trim().to_string();
        if title.is_empty() || title == shown_title(&current, None) {
            return;
        }
        let mut annotation = current.annotation.clone();
        annotation.title = Some(title);
        on_edit.run(Edit::AnnotateDiagram(annotation));
    };
    let delete_project = project_id.clone();
    let delete_id = id.clone();
    let delete = Callback::new(move |_: ()| {
        let (project_id, id) = (delete_project.clone(), delete_id.clone());
        let Some(hash) = file.with_untracked(|f| f.as_ref().map(|f| f.hash.clone())) else {
            return;
        };
        spawn_local(async move {
            match api::delete_diagram(&project_id, &id, &hash).await {
                Ok(()) => {
                    workspace.item.set(None);
                    workspace.refresh_repo().await;
                }
                Err(e) => workspace.fail(e),
            }
        });
    });
    let viewers_id = id.clone();
    let viewers = Memo::new(move |_| {
        workspace.sync.with(|s| {
            s.as_ref()
                .map(|s| {
                    s.presence
                        .iter()
                        .filter(|p| {
                            p.diagram.as_deref() == Some(viewers_id.as_str())
                                && s.me.as_ref() != Some(&p.author)
                        })
                        .map(|p| p.author.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let open_file = {
        let project_id = project_id.clone();
        move || {
            let Some(path) = file.with_untracked(|f| f.as_ref().map(|f| f.path.clone())) else {
                return;
            };
            let project_id = project_id.clone();
            spawn_local(async move {
                if let Err(e) = api::open_code_ref(&project_id, &path).await {
                    workspace.fail(e);
                }
            });
        }
    };
    let deletable = move || file.with(|f| f.as_ref().is_some_and(|f| f.format != "markdown"));
    let inspector_id = id.clone();
    let path_label = id.clone();
    view! {
        <div class="flex h-full min-h-0 flex-col">
            <header class="flex min-h-[58px] shrink-0 items-center gap-3 border-b border-line bg-surface px-[18px]">
                {move || view! { <Icon name=format_icon(&file.with(|f| f.as_ref().map(|f| f.format.clone()).unwrap_or_default())) size=17 class="text-sage" /> }}
                <div class="flex min-w-0 flex-1 flex-col">
                    <input
                        class="min-w-0 truncate rounded border-0 bg-transparent p-0 text-sm font-semibold text-ink focus:bg-field focus:outline-none focus:ring-2 focus:ring-accent/25"
                        aria-label="Diagram title"
                        prop:value=move || diagram.with(|d| file.with(|f| shown_title(d, f.as_ref())))
                        on:change=move |ev| rename(event_target_value(&ev))
                    />
                    <span class="truncate text-[11px] text-muted">
                        <span class="font-mono" title="File in the repository">{path_label}</span>
                        {move || diagram.with(|d| format!(" · {} components · {} connections", d.nodes.len(), d.edges.len()))}
                    </span>
                </div>
                <div class="flex -space-x-1.5">
                    <For each=move || viewers.get() key=|a| a.key.clone() children=|a| view! { <Avatar author=a size=24 /> } />
                </div>
                <super::search_field::SearchField value=query label="Search nodes" placeholder="Find a component…" class="w-[clamp(140px,16vw,220px)]">
                    <Show when=move || !query.with(|q| q.trim().is_empty())>
                        <div class="absolute top-full right-0 z-40 mt-2 max-h-80 w-72 overflow-auto rounded-xl border border-line bg-surface-2 p-1.5 shadow-xl" aria-label="Matching components">
                            {move || {
                                let q = query.get().trim().to_lowercase();
                                let found = diagram.with(|d| d.nodes.iter().filter(|n| crate::label::plain(&n.label).to_lowercase().contains(&q) || n.id.to_lowercase().contains(&q) || n.annotation.tags.iter().any(|t| t.to_lowercase().contains(&q)) || crate::diagram_layout::note(n).is_some_and(|b| b.to_lowercase().contains(&q))).cloned().collect::<Vec<_>>());
                                if found.is_empty() { view! { <p class="p-3 text-xs text-muted">"No matching components"</p> }.into_any() }
                                else { found.into_iter().map(move |node| view! { <button type="button" class="nav-item" on:click=move |_| { query.set(String::new()); selected.set(Some(node.id.clone())); focus_node.set(Some(node.id.clone())); inspecting.set(true); }><Icon name=super::graph::node_card::kind_icon(node.kind()) size=15 /><span class="truncate">{crate::label::plain(&node.label)}</span></button> }).collect_view().into_any() }
                            }}
                        </div>
                    </Show>
                </super::search_field::SearchField>
                {move || freshness.get().filter(|f| !f.nodes.is_empty()).map(|f| {
                    let first = f.nodes.keys().next().cloned();
                    let broken = f.nodes.values().filter(|c| c.status == "broken").count();
                    let count = f.nodes.len();
                    view! {
                        <button type="button" class="flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px]" class=("border-danger-line", broken > 0) class=("text-danger-text", broken > 0) class=("border-warning", broken == 0) class=("text-warning", broken == 0) title=format!("Cited code changed on {} since these nodes were checked", f.branch) on:click=move |_| { if let Some(id) = first.clone() { selected.set(Some(id.clone())); focus_node.set(Some(id)); inspecting.set(true); } }>
                            <Icon name="git-branch" size=12 />
                            {format!("{count} {} · {}", if count == 1 { "node changed" } else { "nodes changed" }, f.branch)}
                        </button>
                    }
                })}
                <Show when=api::desktop>
                    <button type="button" class="icon-btn" aria-label="Local changes" aria-pressed=move || show_local.get().to_string() title=move || if show_local.get() { "Also flagging code changed only in your working copy" } else { "Flag code changed in your working copy too" } on:click=move |_| { let on = !show_local.get_untracked(); show_local.set(on); api::local_storage_set(LOCAL_KEY, if on { "on" } else { "off" }); spawn_local(workspace.refresh_stale()); }><Icon name="git-branch" size=16 /></button>
                </Show>
                <button type="button" class="icon-btn" aria-label="Undo" title="Undo (⌘Z)" on:click=move |_| undo()><Icon name="undo-2" size=16 /></button>
                <button type="button" class="icon-btn" aria-label="Redo" title="Redo (⌘⇧Z)" on:click=move |_| redo()><Icon name="undo-2" size=16 class="-scale-x-100" /></button>
                <Show when=api::desktop>
                    {let open_file = open_file.clone(); view! {
                        <button type="button" class="icon-btn" aria-label="Open file in editor" title="Open the file in your editor" on:click=move |_| open_file()><Icon name="arrow-up-right" size=16 /></button>
                    }}
                </Show>
                <button type="button" class="btn bg-transparent px-3 py-2" on:click=move |_| agent_open.set(true)><Icon name="sparkles" size=15 /> "Ask agent"</button>
                <Show when=deletable>
                    <button type="button" class="icon-btn" aria-label="Delete diagram" title="Delete diagram" on:click=move |_| confirm_delete.set(true)><Icon name="trash-2" size=16 /></button>
                </Show>
            </header>
            <Show when=move || locked.get() && file.with(Option::is_some)>
                <p class="shrink-0 border-b border-line-soft bg-surface-2 px-[18px] py-2 text-[11px] text-muted" role="note">
                    {move || if editable.get() == Editable::None {
                        "Read-only here: this file is exported from another one.".to_string()
                    } else {
                        "The structure is written in the file. Describe, link to code and arrange it here; edit the file or ask your agent for the rest.".to_string()
                    }}
                </p>
            </Show>
            <div class="relative flex min-h-0 flex-1">
                <Show when=move || !load_error.get().is_empty()>
                    <div class="m-auto max-w-[460px] p-6 text-center text-xs leading-relaxed text-danger-text">{move || load_error.get()}</div>
                </Show>
                <Show when=move || file.with(|f| f.as_ref().is_some_and(|f| f.schema.is_some()))>
                    <super::graph::canvas::GraphCanvas
                        source=schema_source
                        query=query
                        namespaces=Signal::derive(Vec::new)
                        related=Signal::derive(|| true)
                        focus_node_ids=Signal::derive(Vec::new)
                        on_select=Callback::new(move |entity: Option<crate::types::Entity>| {
                            selected.set(entity.map(|e| e.id));
                        })
                        on_save=Callback::new(move |positions: HashMap<String, crate::types::Position>| on_edit.run(Edit::Layout(positions)))
                    />
                </Show>
                <Show when=move || draft.with(Option::is_some) && file.with(|f| f.as_ref().is_some_and(|f| f.schema.is_none()))>
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
                        locked=locked
                        on_auto_layout=auto_layout
                        changes=changes
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
                            diagram_id=inspector_id.clone()
                            node=node
                            diagram=current
                            locked=locked.get()
                            change=freshness.with(|f| f.as_ref().and_then(|f| f.nodes.get(&id).cloned()))
                            branch=freshness.with(|f| f.as_ref().map(|f| f.branch.clone()))
                            on_review=review
                            on_update=Callback::new(move |(node, change): (DiagramNode, NodeChange)| {
                                let patches = change.refs.iter().map(|r| format!("{} ({}){}\n{}", r.reference, r.status, r.now.as_ref().map(|n| format!(" → now {n}")).unwrap_or_default(), r.patch.clone().or_else(|| r.reason.clone()).unwrap_or_default())).collect::<Vec<_>>().join("\n\n");
                                ask_agent(workspace, agent_open, format!("The code behind the ‘{}’ node changed since this diagram was checked. Read the changes below and the current code, then update the node (label, note, code references) and its connections in the diagram if they no longer hold. Explain what changed.\n\n{patches}", crate::label::plain(&node.label)));
                            })
                            on_edit=Callback::new(move |edit: Edit| {
                                if matches!(&edit, Edit::Delete { nodes, .. } if !nodes.is_empty()) {
                                    selected.set(None);
                                }
                                on_edit.run(edit);
                            })
                            on_navigate=Callback::new(move |id: String| { selected.set(Some(id.clone())); focus_node.set(Some(id)); })
                            on_explain=Callback::new(move |node: DiagramNode| {
                                let refs = node.annotation.code.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
                                ask_agent(workspace, agent_open, format!("Explain the {} component ‘{}’ in this diagram. Read the relevant code, describe its role and connections, and cite the evidence. Code references: {}", node.kind(), node.label, if refs.is_empty() { "find the relevant code" } else { &refs }));
                            })
                            on_close=Callback::new(move |_: ()| inspecting.set(false))
                        />
                    })
                }}
            </div>
            <Show when=move || confirm_delete.get()>
                <Modal title="Delete this diagram?" on_close=Callback::new(move |_: ()| confirm_delete.set(false))>
                    <p class="text-[13px] leading-[1.9] text-ink">"The file is removed from the repository. Git keeps its history if it was committed."</p>
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

const SOURCES_KEY: &str = "atlas:doc-sources";

#[component]
pub fn DocView(slug: String) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    // Flat docs keep their short slug; docs from the TOC are named by their repository path.
    let doc_path = if slug.ends_with(".md") || slug.ends_with(".markdown") {
        slug.clone()
    } else {
        format!(".schematlas/docs/{slug}.md")
    };
    let embed_path = doc_path.clone();
    workspace.focus.set(Some(crate::state::Focus {
        kind: "doc",
        path: doc_path.clone(),
        node: None,
    }));
    let mine = doc_path.clone();
    on_cleanup(move || clear_focus(workspace, &mine));
    // The sources column can be put away; the choice is remembered for every doc.
    let sources_open =
        RwSignal::new(api::local_storage_get(SOURCES_KEY).as_deref() != Some("hidden"));
    let toggle_sources = move |_| {
        let open = !sources_open.get_untracked();
        sources_open.set(open);
        api::local_storage_set(SOURCES_KEY, if open { "shown" } else { "hidden" });
    };
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
        let disk = workspace
            .toc
            .with(|s| s.hashes.get(&watch_slug).cloned())
            .or_else(|| {
                workspace.repo.with(|r| {
                    r.docs
                        .iter()
                        .find(|d| d.slug == watch_slug || d.path == watch_slug)
                        .map(|d| d.hash.clone())
                })
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
                    <span class="font-mono text-[10px] text-faint">{doc_path.clone()}</span>
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
                        <button type="button" class="icon-btn" aria-label="Sources" aria-pressed=move || sources_open.get().to_string() title=move || if sources_open.get() { "Hide relevant sources" } else { "Show relevant sources" } on:click=toggle_sources><Icon name="panel-left" size=16 class="-scale-x-100" /></button>
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
                <div class="min-h-0 flex-1 overflow-auto px-10 py-8">
                <article class="markdown doc-page">
                    {move || {
                        let (_, parts) = crate::markdown::segments(&text.get());
                        // While editing, Mermaid blocks show as code: the drawn version is the saved one.
                        let live = !editing.get();
                        parts.into_iter().map(|part| match part {
                            Segment::Markdown(md) => view! { <div inner_html=render_markdown(&md)></div> }.into_any(),
                            Segment::Diagram { id, title } => view! { <DiagramPreview id=id title=title /> }.into_any(),
                            Segment::Mermaid { index, source } if live => view! { <DiagramPreview id=format!("{}#{index}", embed_path) source=source /> }.into_any(),
                            Segment::Mermaid { source, .. } => view! { <pre><code>{source}</code></pre> }.into_any(),
                        }).collect_view()
                    }}
                </article>
                </div>
                <aside class="w-[240px] shrink-0 overflow-auto border-l border-line p-4 max-[1100px]:hidden" class=("hidden", move || !sources_open.get()) aria-label="Relevant sources">
                    {move || {
                        let (front, _) = crate::markdown::segments(&text.get());
                        (!front.sources.is_empty() || front.commit.is_some()).then(|| view! {
                            <div class="mb-5">
                                <div class="eyebrow mb-2">"RELEVANT SOURCE FILES"</div>
                                <ul class="flex flex-col gap-1">
                                    {front.sources.into_iter().map(|s| view! { <li class="truncate font-mono text-[10px] text-soft" title=s.clone()>{s.clone()}</li> }).collect_view()}
                                </ul>
                                {front.commit.map(|c| view! { <p class="mt-2 font-mono text-[10px] text-faint" title="Commit this page was written from">{format!("@ {}", c.chars().take(10).collect::<String>())}</p> })}
                            </div>
                        })
                    }}
                    <div class="eyebrow mb-3">"REFERENCES"</div>
                    {move || {
                        let mentions = code_mentions(&text.get());
                        if mentions.is_empty() {
                            view! { <p class="form-hint">"Mention code as `src/file.rs:12` or link a diagram with (schematlas:diagram/.schematlas/diagrams/name.mmd) to list it here."</p> }.into_any()
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
    fn mentions_and_new_diagrams() {
        assert_eq!(
            new_mermaid("Checkout: \"v2\""),
            "---\ntitle: \"Checkout: 'v2'\"\n---\nflowchart LR\n"
        );
        let mentions = code_mentions("See `src/api/mod.rs:12-30`, `cargo test`, [map](schematlas:diagram/arch) and [x](https://e.com).");
        assert_eq!(
            mentions,
            ["src/api/mod.rs:12-30", "schematlas:diagram/arch"]
        );
    }
}
