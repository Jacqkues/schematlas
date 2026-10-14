//! Ordered documentation navigation, backed by `.schematlas/toc.yaml`.
use super::dialogs::{FormError, Modal};
use super::icons::Icon;
use crate::{
    api,
    state::{AtlasItem, Workspace},
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use schematlas_ui::toc::{Edit, Entry, Row};
use std::collections::HashSet;

fn known_entries(workspace: Workspace) -> Vec<Entry> {
    let mut result = workspace.repo.with(|repo| {
        repo.diagrams
            .iter()
            .map(|e| {
                let mut item = Entry::file(e.title.clone(), e.path.clone());
                item.kind = Some("diagram".into());
                // Markdown blocks and draw.io pages are parts of one file.
                match e.format.as_str() {
                    "markdown" => item.block = e.part,
                    "drawio" => item.page = e.part,
                    _ => {}
                }
                item
            })
            .chain(
                repo.docs
                    .iter()
                    .map(|e| Entry::file(e.title.clone(), e.path.clone())),
            )
            .collect::<Vec<_>>()
    });
    if let Some(project) = workspace.project.get() {
        result.extend(
            project
                .sources
                .iter()
                .map(|s| Entry::source(s.name.clone(), s.name.clone(), s.kind.clone())),
        );
    }
    result
}
fn same_target(a: &Entry, b: &Entry) -> bool {
    a.file == b.file
        && a.source == b.source
        && a.block == b.block
        && a.page == b.page
        && (a.source.is_none() || a.kind == b.kind)
}
fn icon(entry: &Entry) -> &'static str {
    if entry.is_folder() {
        "folder"
    } else if entry.is_markdown() {
        "file-text"
    } else if entry.kind.as_deref() == Some("database")
        || entry.file.as_ref().is_some_and(|f| f.ends_with(".dbml"))
    {
        "database"
    } else if entry.kind.as_deref() == Some("openapi") {
        "braces"
    } else {
        "workflow"
    }
}
fn active(workspace: Workspace, entry: &Entry) -> bool {
    if let Some(source) = &entry.source {
        return workspace.source.with(|s| {
            s.as_ref()
                .is_some_and(|s| s.name == *source && Some(&s.kind) == entry.kind.as_ref())
        });
    }
    let Some(file) = &entry.file else {
        return false;
    };
    match workspace.item.get() {
        Some(AtlasItem::Doc(slug)) => {
            file == &slug || file == &format!(".schematlas/docs/{slug}.md")
        }
        Some(AtlasItem::Diagram(id)) => entry.document_id().is_some_and(|d| d == id),
        None => false,
    }
}
/// The only navigation adapter that depends on the diagram listing's identity model.
pub fn open_entry(workspace: Workspace, entry: &Entry) {
    if let Some(name) = &entry.source {
        let matches = workspace.project.with_untracked(|p| {
            p.as_ref()
                .map(|p| {
                    p.sources
                        .iter()
                        .filter(|s| s.name == *name && Some(&s.kind) == entry.kind.as_ref())
                        .map(|s| s.id.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
        if matches.len() == 1 {
            workspace.select_source(matches[0].clone());
        } else {
            workspace.fail(format!(
                "Connect exactly one source named “{name}” in this workspace."
            ));
        }
    } else if entry.is_markdown() {
        if let Some(file) = &entry.file {
            workspace.open_item(AtlasItem::Doc(file.clone()));
        }
    } else if let (Some(file), Some(id)) = (&entry.file, entry.document_id()) {
        // A draw.io file named without a page opens its first page.
        let found = workspace.repo.with_untracked(|repo| {
            [id.clone(), format!("{id}#0")]
                .into_iter()
                .find(|candidate| repo.diagrams.iter().any(|e| &e.id == candidate))
        });
        if let Some(id) = found {
            workspace.open_item(AtlasItem::Diagram(id));
        } else {
            workspace.fail(format!("Cannot open “{file}”. Check that it exists and its format is available in this build."));
        }
    }
}

#[derive(Clone)]
enum Dialog {
    Create(Option<String>),
    Folder(Row),
    Move(Row),
    Add(Entry),
}
#[derive(Clone, Copy)]
struct Tree {
    rows: Memo<Vec<Row>>,
    closed: RwSignal<HashSet<String>>,
    selected: RwSignal<Option<String>>,
    dialog: RwSignal<Option<Dialog>>,
}

#[component]
pub fn ProjectExplorer(
    #[prop(into)] on_new_diagram: Callback<()>,
    #[prop(into)] on_new_doc: Callback<()>,
    #[prop(into)] on_connect: Callback<()>,
    #[prop(into)] on_import: Callback<()>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let tree = Tree {
        rows: Memo::new(move |_| workspace.toc.with(|s| s.toc.rows())),
        closed: RwSignal::new(HashSet::new()),
        selected: RwSignal::new(None),
        dialog: RwSignal::new(None),
    };
    let ready = move || {
        workspace
            .toc
            .with(|s| s.root.is_some() && s.error.is_none())
    };
    let unlisted = Memo::new(move |_| {
        let rows = tree.rows.get();
        known_entries(workspace)
            .into_iter()
            .filter(|e| !rows.iter().any(|r| same_target(e, &r.entry)))
            .collect::<Vec<_>>()
    });
    let busy = RwSignal::new(false);
    Effect::new(move |_| {
        workspace.project_id.track();
        tree.closed.set(HashSet::new());
        tree.selected.set(None);
        tree.dialog.set(None);
    });
    Effect::new(move |_| {
        let rows = tree.rows.get();
        if let Some(row) = rows.iter().find(|r| active(workspace, &r.entry)) {
            let toc = workspace.toc.with_untracked(|s| s.toc.clone());
            tree.closed
                .update(|closed| closed.retain(|id| !toc.within(&row.id, id)));
        }
    });
    let initialize = move |_| {
        let Some(id) = workspace.project_id.get_untracked() else {
            return;
        };
        let base = workspace.toc.with_untracked(|s| s.hash.clone());
        busy.set(true);
        spawn_local(async move {
            match api::edit_toc(&id, None, base.as_deref()).await {
                Ok(snapshot) => {
                    if workspace.project_id.get_untracked().as_deref() == Some(&id) {
                        workspace.toc.set(snapshot);
                    }
                }
                Err(e) => workspace.fail(e),
            }
            busy.set(false);
        });
    };
    view! {
        <div class="mt-5 mb-1 flex h-8 items-center justify-between pl-2 pr-1">
            <button type="button" class="quiet-heading hover:text-ink" title="Project root" on:click=move |_| tree.selected.set(None)>"Explorer"</button>
            <div class="flex items-center gap-0.5">
                <button type="button" class="icon-btn size-7" aria-label="New doc" title="New document" disabled=move || !ready() on:click=move |_| on_new_doc.run(())><Icon name="file-text" size=14 /></button>
                <button type="button" class="icon-btn size-7" aria-label="New diagram" title="New diagram" disabled=move || !ready() on:click=move |_| on_new_diagram.run(())><Icon name="workflow" size=14 /></button>
                <button type="button" class="icon-btn size-7" aria-label="New folder" title="New folder" disabled=move || !ready() on:click=move |_| tree.dialog.set(Some(Dialog::Create(tree.selected.get_untracked())))><Icon name="folder-plus" size=15 /></button>
                <button type="button" class="icon-btn size-7" aria-label="Collapse all folders" title="Collapse all folders" on:click=move |_| tree.closed.set(tree.rows.with_untracked(|r| r.iter().filter(|r| r.entry.is_folder()).map(|r| r.id.clone()).collect()))><Icon name="collapse-all" size=15 /></button>
            </div>
        </div>
        <Show when=move || workspace.toc.with(|s| s.error.is_some())>
            <div class="mx-1 mb-3 rounded-md border border-danger-line p-3 text-xs" role="alert">
                <p class="leading-5 text-danger-text">{move || workspace.toc.with(|s| s.error.clone().unwrap_or_default())}</p>
                <button type="button" class="mt-2 text-muted underline" on:click=move |_| spawn_local(workspace.refresh_toc())>"Reload toc.yaml"</button>
            </div>
        </Show>
        <Show when=move || ready() && workspace.toc.with(|s| s.hash.is_none())>
            <div class="mx-1 mb-3 rounded-md border border-line-soft p-2.5 text-[11px] leading-5 text-muted">
                <p>"Save this documentation structure with your repository."</p>
                <button type="button" class="mt-1 font-medium text-ink underline" disabled=move || busy.get() on:click=initialize>"Create toc.yaml"</button>
            </div>
        </Show>
        <nav class="min-h-0 flex-1 overflow-auto pb-4" aria-label="Project files">
            <TocBranch tree=tree parent=None depth=0 />
            <Show when=move || ready() && tree.rows.with(Vec::is_empty)><p class="px-2 py-3 text-xs leading-5 text-muted">"Add a document or create your first folder."</p></Show>
            <Show when=move || !unlisted.with(Vec::is_empty)>
                <details class="mt-4 border-t border-line-soft pt-3" open>
                    <summary class="cursor-pointer px-2 text-[11px] text-muted">"Not in TOC"</summary>
                    <ul class="mt-2 space-y-0.5">
                        <For each=move || unlisted.get() key=|e| (e.file.clone(), e.source.clone(), e.title.clone()) children=move |entry| {
                            let opening = entry.clone(); let adding = entry.clone();
                            view! { <li class="group flex h-8 items-center rounded-md pr-1 hover:bg-surface-3"><button type="button" class="flex h-full min-w-0 flex-1 items-center gap-2 px-2 text-left text-xs" on:click=move |_| open_entry(workspace, &opening)><Icon name=icon(&entry) size=14 class="text-muted" /><span class="truncate">{entry.title.clone()}</span></button><button type="button" class="explorer-action" aria-label=format!("Add {} to TOC", entry.title) title="Add to documentation" disabled=move || !ready() on:click=move |_| tree.dialog.set(Some(Dialog::Add(adding.clone())))><Icon name="plus" size=14 /></button></li> }
                        } />
                    </ul>
                </details>
            </Show>
            <div class="mt-4 flex flex-wrap gap-1 border-t border-line-soft pt-2">
                <button type="button" class="icon-btn size-7" aria-label="Connect database" title="Connect database" on:click=move |_| on_connect.run(())><Icon name="database" size=14 /></button>
                <button type="button" class="icon-btn size-7" aria-label="Import OpenAPI" title="Import OpenAPI" on:click=move |_| on_import.run(())><Icon name="braces" size=14 /></button>
            </div>
        </nav>
        {move || tree.dialog.get().map(|dialog| view! { <ExplorerDialog tree=tree dialog=dialog /> })}
    }
}

#[component]
fn TocBranch(tree: Tree, parent: Option<String>, depth: usize) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let rows = Memo::new(move |_| {
        tree.rows.with(|rows| {
            rows.iter()
                .filter(|r| r.parent == parent)
                .cloned()
                .collect::<Vec<_>>()
        })
    });
    view! {
        <ul class=if depth == 0 { "space-y-0.5" } else { "ml-[15px] space-y-0.5 border-l border-line-soft pl-2" }>
            <For each=move || rows.get() key=|r| (r.id.clone(), r.entry.title.clone()) children=move |row| {
                let stored = StoredValue::new(row.clone());
                let folder = row.entry.is_folder();
                let open = move || !tree.closed.with(|s| s.contains(&stored.get_value().id));
                let selected = move || if folder { tree.selected.get().as_ref() == Some(&stored.get_value().id) } else { active(workspace, &stored.get_value().entry) };
                let issue = move || workspace.toc.with(|s| s.issues.get(&stored.get_value().id).cloned());
                view! { <li>
                    <div class="group flex h-8 items-center rounded-md pr-1 hover:bg-surface-3 focus-within:bg-surface-3" class=("bg-surface-4", selected)>
                        <button type="button" class="flex h-full min-w-0 flex-1 items-center gap-1.5 rounded-md px-1 text-left text-xs text-text" aria-expanded=move || folder.then(|| open().to_string()) aria-current=move || (!folder && selected()).then_some("page") title=move || issue().unwrap_or_else(|| stored.get_value().entry.title)
                            on:click=move |_| {
                                let row = stored.get_value();
                                if folder { tree.selected.set(Some(row.id.clone())); tree.closed.update(|s| { if !s.remove(&row.id) { s.insert(row.id); } }); }
                                else if let Some(error) = issue() { workspace.fail(error); }
                                else { tree.selected.set(None); open_entry(workspace, &row.entry); }
                            }
                            on:keydown=move |ev| { if folder { match ev.key().as_str() { "ArrowRight" => { ev.prevent_default(); tree.closed.update(|s| { s.remove(&stored.get_value().id); }); }, "ArrowLeft" => { ev.prevent_default(); tree.closed.update(|s| { s.insert(stored.get_value().id); }); }, _ => {} } } }>
                            <span class="w-3 text-muted" class=("-rotate-90", move || folder && !open())>{folder.then(|| view! { <Icon name="chevron-down" size=12 /> })}</span>
                            {move || view! { <Icon name=if folder && open() { "folder-open" } else { icon(&stored.get_value().entry) } size=14 class="text-muted" /> }}
                            <span class="truncate">{row.entry.title.clone()}</span>
                            <Show when=move || issue().is_some()><Icon name="circle-alert" size=12 class="text-danger-text" /></Show>
                        </button>
                        <button type="button" class="explorer-action" aria-label=if folder { format!("Manage {} folder", row.entry.title) } else { format!("Move {}", row.entry.title) } title=if folder { "Folder options" } else { "Move to folder" } on:click=move |_| tree.dialog.set(Some(if folder { Dialog::Folder(stored.get_value()) } else { Dialog::Move(stored.get_value()) }))><Icon name="ellipsis" size=14 /></button>
                    </div>
                    <Show when=move || folder && open()>
                        {if depth < schematlas_ui::toc::MAX_DEPTH { view! { <TocBranch tree=tree parent=Some(row.id.clone()) depth=depth+1 /> }.into_any() } else { ().into_any() }}
                    </Show>
                </li> }
            } />
            <Show when=move || (depth > 0) && rows.with(Vec::is_empty)><li class="py-1.5 pl-[19px] text-[11px] text-faint">"Empty folder"</li></Show>
        </ul>
    }
}

#[component]
pub fn FolderSelect(
    value: RwSignal<String>,
    #[prop(optional_no_strip)] exclude: Option<String>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    view! {
        <label class="form-label" for="destination-folder">"Location"</label>
        <super::select::Select id="destination-folder" label="Location" class="mb-[21px]" value=Signal::derive(move || value.get()) on_change=Callback::new(move |v| value.set(v)) options=Signal::derive(move || {
            let toc = workspace.toc.with(|s| s.toc.clone());
            std::iter::once((String::new(), "Project root".into())).chain(toc.rows().into_iter().filter(|r| r.entry.is_folder() && !exclude.as_ref().is_some_and(|id| toc.within(&r.id, id))).map(|r| (r.id, r.path))).collect()
        }) />
    }
}

#[component]
fn ExplorerDialog(tree: Tree, dialog: Dialog) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let project_id = workspace.project_id.get_untracked().unwrap_or_default();
    // Capture the hash that the user actually reviewed, including while a file-watcher refreshes.
    let base_hash = workspace.toc.with_untracked(|s| s.hash.clone());
    let (heading, title, location, row) = match &dialog {
        Dialog::Create(parent) => ("New folder", String::new(), parent.clone(), None),
        Dialog::Folder(row) => (
            "Folder options",
            row.entry.title.clone(),
            row.parent.clone(),
            Some(row.clone()),
        ),
        Dialog::Move(row) => (
            "Move to folder",
            row.entry.title.clone(),
            row.parent.clone(),
            Some(row.clone()),
        ),
        Dialog::Add(entry) => ("Add to documentation", entry.title.clone(), None, None),
    };
    let name = RwSignal::new(title);
    let parent = RwSignal::new(location.unwrap_or_default());
    let before = RwSignal::new(String::new());
    Effect::watch(
        move || parent.get(),
        move |_, _, _| before.set(String::new()),
        false,
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let row = StoredValue::new(row);
    let editing_folder = matches!(dialog, Dialog::Folder(_));
    let moving = matches!(dialog, Dialog::Move(_) | Dialog::Add(_));
    let save = Callback::new(move |edit: Edit| {
        let id = project_id.clone();
        let base_hash = base_hash.clone();
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            match api::edit_toc(&id, Some(&edit), base_hash.as_deref()).await {
                Ok(snapshot) => {
                    if workspace.project_id.get_untracked().as_deref() == Some(&id) {
                        workspace.toc.set(snapshot);
                        tree.closed.set(HashSet::new());
                        tree.selected.set(None);
                        tree.dialog.set(None);
                    }
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    });
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let value = parent.get_untracked();
        let parent = (!value.is_empty()).then_some(value);
        let value = before.get_untracked();
        let before = (!value.is_empty()).then_some(value);
        save.run(match &dialog {
            Dialog::Create(_) => Edit::Create {
                title: name.get_untracked(),
                parent,
            },
            Dialog::Folder(row) => Edit::Update {
                id: row.id.clone(),
                title: name.get_untracked(),
                parent,
            },
            Dialog::Move(row) => Edit::Move {
                id: row.id.clone(),
                parent,
                before,
            },
            Dialog::Add(entry) => Edit::Add {
                entry: entry.clone(),
                parent,
            },
        });
    };
    view! {
        <Modal title=heading on_close=Callback::new(move |_: ()| tree.dialog.set(None)) busy=busy>
            <form on:submit=submit>
                <Show when=move || !moving fallback=move || view! { <p class="mb-5 text-sm text-muted">{move || name.get()}</p> }>
                    <label class="form-label" for="folder-name">"Folder name"</label><input id="folder-name" class="field mb-[21px]" bind:value=name required maxlength="120" autofocus placeholder="e.g. Checkout" />
                </Show>
                <FolderSelect value=parent exclude=if editing_folder { row.get_value().map(|r| r.id) } else { None } />
                <Show when=move || moving && row.get_value().is_some()>
                    <label class="form-label" for="toc-before">"Place before"</label>
                    <super::select::Select id="toc-before" label="Place before" class="mb-[21px]" value=Signal::derive(move || before.get()) on_change=Callback::new(move |v| before.set(v)) options=Signal::derive(move || {
                        let value = parent.get(); let parent = (!value.is_empty()).then_some(value);
                        std::iter::once((String::new(), "End of folder".into())).chain(tree.rows.get().into_iter().filter(|r| r.parent == parent && row.get_value().is_none_or(|current| current.id != r.id)).map(|r| (r.id, r.entry.title))).collect()
                    }) />
                </Show>
                <FormError error=error />
                <Show when=move || moving && row.get_value().is_some()>
                    <button type="button" class="btn btn-ghost-danger mb-5" disabled=move || busy.get() on:click=move |_| { if let Some(row) = row.get_value() { save.run(Edit::Remove { id: row.id }); } }>"Remove from TOC"</button>
                    <p class="mb-5 text-xs text-muted">"Removing an entry keeps its file in the repository."</p>
                </Show>
                <Show when=move || editing_folder>
                    <div class="mb-5 flex gap-2"><button type="button" class="btn" disabled=move || busy.get() on:click=move |_| tree.dialog.set(Some(Dialog::Create(row.get_value().map(|r| r.id))))><Icon name="folder-plus" size=14 />"New subfolder"</button><button type="button" class="btn btn-ghost-danger" disabled=move || busy.get() || row.get_value().is_none_or(|r| r.entry.children.is_none_or(|c| !c.is_empty())) title="Only empty folders can be deleted" on:click=move |_| { if let Some(row) = row.get_value() { save.run(Edit::Remove { id: row.id }); } }><Icon name="trash-2" size=14 />"Delete folder"</button></div>
                </Show>
                <div class="modal-footer"><button type="button" class="btn" disabled=move || busy.get() on:click=move |_| tree.dialog.set(None)>"Cancel"</button><button type="submit" class="btn btn-primary" disabled=move || busy.get()>{move || if busy.get() { "Saving…" } else if moving { "Move" } else if editing_folder { "Save folder" } else { "Create folder" }}</button></div>
            </form>
        </Modal>
    }
}
