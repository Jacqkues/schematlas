//! The project file explorer. Folders organize references to documents and data sources.
use super::dialogs::{FormError, Modal};
use super::icons::Icon;
use crate::api;
use crate::state::{AtlasItem, Workspace};
use leptos::prelude::*;
use leptos::task::spawn_local;
use schematlas_ui::explorer::{Edit, Explorer as Organization, Folder};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Item {
    id: String,
    title: String,
    default: &'static str,
    icon: &'static str,
    error: bool,
}

fn items(workspace: Workspace) -> Vec<Item> {
    let mut items = workspace.repo.with(|repo| {
        repo.diagrams
            .iter()
            .map(|e| Item {
                id: format!("diagram:{}", e.slug),
                title: e.title.clone(),
                default: "diagrams",
                icon: "workflow",
                error: e.error.is_some(),
            })
            .chain(repo.docs.iter().map(|e| Item {
                id: format!("doc:{}", e.slug),
                title: e.title.clone(),
                default: "docs",
                icon: "file-text",
                error: e.error.is_some(),
            }))
            .collect::<Vec<_>>()
    });
    if let Some(project) = workspace.project.get() {
        items.extend(project.sources.iter().map(|s| Item {
            id: format!("source:{}", s.id),
            title: s.name.clone(),
            default: if s.is_database() { "databases" } else { "apis" },
            icon: if s.is_database() {
                "database"
            } else {
                "braces"
            },
            error: false,
        }));
    }
    items.sort_by_key(|i| i.title.to_lowercase());
    items
}

fn selected_item(workspace: Workspace) -> Option<String> {
    match workspace.item.get() {
        Some(AtlasItem::Diagram(slug)) => Some(format!("diagram:{slug}")),
        Some(AtlasItem::Doc(slug)) => Some(format!("doc:{slug}")),
        None => workspace.source_id.get().map(|id| format!("source:{id}")),
    }
}

#[derive(Clone)]
enum Dialog {
    Create(Option<String>),
    Folder(Folder),
    Move(Item),
}

#[derive(Clone, Copy)]
struct Tree {
    organization: Memo<Organization>,
    items: Memo<Vec<Item>>,
    closed: RwSignal<HashSet<String>>,
    selected: RwSignal<Option<String>>,
    dialog: RwSignal<Option<Dialog>>,
    create: Callback<&'static str>,
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
        organization: Memo::new(move |_| {
            workspace
                .project
                .with(|p| p.as_ref().map(|p| p.explorer.clone()).unwrap_or_default())
        }),
        items: Memo::new(move |_| items(workspace)),
        closed: RwSignal::new(HashSet::new()),
        selected: RwSignal::new(None),
        dialog: RwSignal::new(None),
        create: Callback::new(move |kind| match kind {
            "diagrams" => on_new_diagram.run(()),
            "docs" => on_new_doc.run(()),
            "databases" => on_connect.run(()),
            _ => on_import.run(()),
        }),
    };
    Effect::new(move |_| {
        workspace.project_id.track();
        tree.closed.set(HashSet::new());
        tree.selected.set(None);
        tree.dialog.set(None);
    });
    // Reveal a file opened from search, a link, or the project overview.
    Effect::new(move |_| {
        let current = selected_item(workspace);
        let organization = tree.organization.get();
        let all = tree.items.get();
        if let Some(item) = all.iter().find(|i| Some(&i.id) == current.as_ref()) {
            let folder = organization.location(&item.id, item.default);
            tree.closed.update(|closed| {
                if let Some(folder) = folder {
                    closed.retain(|id| !organization.within(&folder, id));
                }
            });
        }
    });
    view! {
        <div class="mt-5 mb-1 flex h-8 items-center justify-between pl-2 pr-1">
            <button type="button" class="quiet-heading hover:text-ink" title="Project root" on:click=move |_| tree.selected.set(None)>"Explorer"</button>
            <div class="flex items-center gap-0.5">
                <button type="button" class="icon-btn size-7" aria-label="New folder" title="New folder" on:click=move |_| tree.dialog.set(Some(Dialog::Create(tree.selected.get_untracked())))><Icon name="folder-plus" size=15 /></button>
                <button type="button" class="icon-btn size-7" aria-label="Collapse all folders" title="Collapse all folders" on:click=move |_| tree.closed.set(tree.organization.with_untracked(|o| o.folders.iter().map(|f| f.id.clone()).collect()))><Icon name="collapse-all" size=15 /></button>
            </div>
        </div>
        <nav class="min-h-0 flex-1 overflow-auto pb-4" aria-label="Project files">
            <ExplorerBranch tree=tree parent=None depth=0 />
        </nav>
        {move || tree.dialog.get().map(|dialog| view! { <ExplorerDialog tree=tree dialog=dialog /> })}
    }
}

#[component]
fn ExplorerBranch(tree: Tree, parent: Option<String>, depth: usize) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let file_parent = parent.clone();
    let folders = Memo::new(move |_| {
        let mut folders = tree.organization.with(|o| {
            o.folders
                .iter()
                .filter(|f| f.parent == parent)
                .cloned()
                .collect::<Vec<_>>()
        });
        folders.sort_by_key(|f| f.name.to_lowercase());
        folders
    });
    let files = Memo::new(move |_| {
        tree.items.with(|items| {
            tree.organization.with(|o| {
                items
                    .iter()
                    .filter(|i| o.location(&i.id, i.default) == file_parent)
                    .cloned()
                    .collect::<Vec<_>>()
            })
        })
    });
    view! {
        <ul class=if depth == 0 { "space-y-0.5" } else { "ml-[15px] space-y-0.5 border-l border-line-soft pl-2" }>
            <For each=move || folders.get() key=|f| (f.id.clone(), f.name.clone()) children=move |folder| {
                let id = StoredValue::new(folder.id.clone());
                let open = move || !tree.closed.with(|closed| closed.contains(&id.get_value()));
                let selected = move || tree.selected.get().as_ref() == Some(&id.get_value());
                let label = folder.name.clone();
                let edit_folder = folder.clone();
                let action = match folder.id.as_str() {
                    "diagrams" => Some(("diagrams", "New diagram")),
                    "docs" => Some(("docs", "New doc")),
                    "databases" => Some(("databases", "Connect database")),
                    "apis" => Some(("apis", "Import OpenAPI")),
                    _ => None,
                };
                view! {
                    <li>
                        <div class="group flex h-8 items-center rounded-md pr-1 hover:bg-surface-3 focus-within:bg-surface-3" class=("bg-surface-3", selected)>
                            <button type="button" class="flex h-full min-w-0 flex-1 items-center gap-1.5 rounded-md px-1 text-left text-xs text-text" aria-expanded=move || open().to_string() title=label.clone()
                                on:click=move |_| {
                                    tree.selected.set(Some(id.get_value()));
                                    tree.closed.update(|closed| { if !closed.remove(&id.get_value()) { closed.insert(id.get_value()); } });
                                }
                                on:keydown=move |ev| match ev.key().as_str() {
                                    "ArrowRight" => { ev.prevent_default(); tree.closed.update(|s| { s.remove(&id.get_value()); }); },
                                    "ArrowLeft" => { ev.prevent_default(); tree.closed.update(|s| { s.insert(id.get_value()); }); },
                                    _ => {}
                                }>
                                <span class="text-muted" class=("-rotate-90", move || !open())><Icon name="chevron-down" size=12 /></span>
                                {move || view! { <Icon name=if open() { "folder-open" } else { "folder" } size=15 class="text-muted" /> }}
                                <span class="truncate">{label.clone()}</span>
                            </button>
                            {action.map(|(kind, label)| view! { <button type="button" class="explorer-action" aria-label=label title=label on:click=move |_| tree.create.run(kind)><Icon name="plus" size=13 /></button> })}
                            <button type="button" class="explorer-action" aria-label=format!("Manage {} folder", folder.name) title="Folder options" on:click=move |_| tree.dialog.set(Some(Dialog::Folder(edit_folder.clone())))><Icon name="ellipsis" size=14 /></button>
                        </div>
                        <Show when=open>
                            {if depth < 8 { view! { <ExplorerBranch tree=tree parent=Some(folder.id.clone()) depth=depth+1 /> }.into_any() } else { ().into_any() }}
                        </Show>
                    </li>
                }
            } />
            <For each=move || files.get() key=|i| (i.id.clone(), i.title.clone(), i.error) children=move |item| {
                let id = StoredValue::new(item.id.clone());
                let active = move || selected_item(workspace).as_ref() == Some(&id.get_value());
                let moving = item.clone();
                view! {
                    <li class="group flex h-8 items-center rounded-md pr-1 hover:bg-surface-3 focus-within:bg-surface-3" class=("bg-surface-4", active)>
                        <button type="button" class="flex h-full min-w-0 flex-1 items-center gap-2 rounded-md pr-1 pl-[19px] text-left text-xs text-text" aria-current=move || active().then_some("page") title=item.title.clone()
                            on:click=move |_| {
                                tree.selected.set(None);
                                let value = id.get_value();
                                if let Some(slug) = value.strip_prefix("diagram:") { workspace.open_item(AtlasItem::Diagram(slug.into())); }
                                else if let Some(slug) = value.strip_prefix("doc:") { workspace.open_item(AtlasItem::Doc(slug.into())); }
                                else if let Some(source) = value.strip_prefix("source:") { workspace.select_source(source.into()); }
                            }>
                            <Icon name=item.icon size=14 class="text-muted" /><span class="truncate">{item.title.clone()}</span>
                            {item.error.then(|| view! { <Icon name="circle-alert" size=12 class="text-danger-text" /> })}
                        </button>
                        <button type="button" class="explorer-action" aria-label=format!("Move {}", item.title) title="Move to folder" on:click=move |_| tree.dialog.set(Some(Dialog::Move(moving.clone())))><Icon name="ellipsis" size=14 /></button>
                    </li>
                }
            } />
            <Show when=move || (depth > 0) && folders.with(Vec::is_empty) && files.with(Vec::is_empty)>
                <li class="py-1.5 pl-[19px] text-[11px] text-faint">"Empty folder"</li>
            </Show>
        </ul>
    }
}

/// Shared picker for new documents and explorer operations.
#[component]
pub fn FolderSelect(
    value: RwSignal<String>,
    #[prop(optional_no_strip)] exclude: Option<String>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    view! {
        <label class="form-label" for="destination-folder">"Location"</label>
        <select id="destination-folder" class="field mb-[21px]" on:change=move |ev| value.set(event_target_value(&ev))>
            <option value="" prop:selected=move || value.get().is_empty()>"Project root"</option>
            {move || {
                let organization = workspace.project.with(|p| p.as_ref().map(|p| p.explorer.clone()).unwrap_or_default());
                let mut options = organization.folders.iter().filter(|f| !exclude.as_ref().is_some_and(|id| organization.within(&f.id, id))).map(|f| (f.id.clone(), organization.path(&f.id))).collect::<Vec<_>>();
                options.sort_by_key(|(_, path)| path.to_lowercase());
                options.into_iter().map(|(id, path)| { let selected = id.clone(); view! { <option value=id prop:selected=move || value.get() == selected>{path}</option> } }).collect_view()
            }}
        </select>
    }
}

#[component]
fn ExplorerDialog(tree: Tree, dialog: Dialog) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let project_id = workspace.project_id.get_untracked().unwrap_or_default();
    let (heading, name, parent, folder_id, item_id) = match &dialog {
        Dialog::Create(parent) => ("New folder", String::new(), parent.clone(), None, None),
        Dialog::Folder(folder) => (
            "Folder options",
            folder.name.clone(),
            folder.parent.clone(),
            Some(folder.id.clone()),
            None,
        ),
        Dialog::Move(item) => (
            "Move to folder",
            item.title.clone(),
            tree.organization
                .with_untracked(|o| o.location(&item.id, item.default)),
            None,
            Some(item.id.clone()),
        ),
    };
    let name = RwSignal::new(name);
    let parent = RwSignal::new(parent.unwrap_or_default());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let folder_id = StoredValue::new(folder_id);
    let moving = item_id.is_some();
    let save = Callback::new(move |edit: Edit| {
        let project_id = project_id.clone();
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            match api::edit_explorer(&project_id, &edit).await {
                Ok(project) => {
                    workspace.replace(project);
                    if workspace.project_id.get_untracked().as_deref() == Some(&project_id) {
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
        let parent = parent.get_untracked();
        let parent = (!parent.is_empty()).then_some(parent);
        save.run(if let Some(item) = &item_id {
            Edit::Move {
                item: item.clone(),
                folder: parent,
            }
        } else if let Some(id) = folder_id.get_value() {
            Edit::Update {
                id,
                name: name.get_untracked(),
                parent,
            }
        } else {
            Edit::Create {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.get_untracked(),
                parent,
            }
        });
    };
    let can_delete = move || {
        folder_id.get_value().is_some_and(|id| {
            tree.organization.with(|o| {
                !o.folders.iter().any(|f| f.parent.as_ref() == Some(&id))
                    && !tree.items.with(|items| {
                        items
                            .iter()
                            .any(|i| o.location(&i.id, i.default).as_ref() == Some(&id))
                    })
            })
        })
    };
    view! {
        <Modal title=heading on_close=Callback::new(move |_: ()| tree.dialog.set(None)) busy=busy>
            <form on:submit=submit>
                <Show when=move || !moving fallback=move || view! { <p class="mb-5 text-sm text-muted">{move || name.get()}</p> }>
                    <label class="form-label" for="folder-name">"Folder name"</label>
                    <input id="folder-name" class="field mb-[21px]" bind:value=name required maxlength="80" autofocus placeholder="e.g. Checkout" />
                </Show>
                <FolderSelect value=parent exclude=folder_id.get_value() />
                <FormError error=error />
                <Show when=move || folder_id.get_value().is_some()>
                    <div class="mb-5 flex items-center gap-2">
                        <button type="button" class="btn" disabled=move || busy.get() on:click=move |_| tree.dialog.set(Some(Dialog::Create(folder_id.get_value())))><Icon name="folder-plus" size=14 />"New subfolder"</button>
                        <button type="button" class="btn btn-ghost-danger" disabled=move || busy.get() || !can_delete() title=move || if can_delete() { "Delete empty folder" } else { "Move this folder’s contents elsewhere first" } on:click=move |_| { if let Some(id) = folder_id.get_value() { save.run(Edit::Remove { id }); } }><Icon name="trash-2" size=14 />"Delete folder"</button>
                    </div>
                </Show>
                <div class="modal-footer">
                    <button type="button" class="btn" disabled=move || busy.get() on:click=move |_| tree.dialog.set(None)>"Cancel"</button>
                    <button type="submit" class="btn btn-primary" disabled=move || busy.get()>{move || if busy.get() { "Saving…" } else if moving { "Move" } else if folder_id.get_value().is_some() { "Save folder" } else { "Create folder" }}</button>
                </div>
            </form>
        </Modal>
    }
}
