//! Named, colored domain groups: the list in the map options popover and the create/edit dialog.
use super::dialogs::{FormError, Modal};
use super::icons::Icon;
use crate::api;
use crate::layout::DEFAULT_GROUP_COLOR;
use crate::types::{CanvasGroup, Project, Source};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;

const PALETTE: [(&str, &str); 6] = [
    ("Slate", "#879b91"),
    ("Blue", "#6d9de3"),
    ("Violet", "#ac8cda"),
    ("Rose", "#d48b9e"),
    ("Amber", "#c9a369"),
    ("Teal", "#6eaaa9"),
];

#[component]
pub fn CanvasGroups(
    #[prop(into)] source: Signal<Arc<Source>>,
    #[prop(into)] project_id: Signal<String>,
    #[prop(into)] on_update: Callback<Project>,
    #[prop(into)] on_focus: Callback<Vec<String>>,
) -> impl IntoView {
    // None: closed. Some(None): create. Some(Some(group)): edit.
    let editing = RwSignal::new(None::<Option<CanvasGroup>>);
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let change = move |undo: bool, group_id: Option<String>| {
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let (project_id, source_id) = (project_id.get_untracked(), source.get_untracked().id.clone());
            let result = if undo {
                api::undo_canvas(&project_id, &source_id).await
            } else {
                api::remove_group(&project_id, &source_id, group_id.as_deref().unwrap_or_default()).await
            };
            match result {
                Ok(project) => on_update.run(project),
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    let groups = move || source.with(|s| s.groups().to_vec());
    view! {
        <div class="p-3">
            <div class="mb-2.5 flex justify-between">
                <button
                    type="button"
                    class="btn"
                    disabled=move || busy.get() || source.with(|s| s.graph.entities.is_empty())
                    on:click=move |_| editing.set(Some(None))
                >
                    <Icon name="plus" size=13 /> "New group"
                </button>
                <Show when=move || source.with(|s| s.layout_backup.is_some())>
                    <button type="button" class="icon-btn" aria-label="Undo canvas edit" disabled=move || busy.get() on:click=move |_| change(true, None)>
                        <Icon name="undo-2" size=15 />
                    </button>
                </Show>
            </div>
            <Show
                when=move || !groups().is_empty()
                fallback=|| view! { <p class="text-xs leading-relaxed text-muted">"No groups yet. Create one to organize related nodes."</p> }
            >
                <For each=groups key=|g| (g.id.clone(), g.name.clone(), g.color.clone()) children=move |group| {
                    let color = crate::layout::valid_color(group.color.as_deref());
                    let focus_ids = group.node_ids.clone();
                    let edit_group = group.clone();
                    let remove_id = group.id.clone();
                    view! {
                        <div class="flex items-center gap-0.5 border-t border-line py-[7px]">
                            <button
                                type="button"
                                class="flex min-w-0 flex-1 items-center gap-2 px-1 py-1.5 text-left text-[11px] text-text transition-colors hover:text-ink"
                                aria-label=format!("Focus group {}", group.name)
                                on:click=move |_| on_focus.run(focus_ids.clone())
                            >
                                <span class="size-1.5 shrink-0 rounded-full" style:background=color></span>
                                <span class="flex-1">{group.name.clone()}</span>
                                <Icon name="scan" size=13 />
                            </button>
                            <button type="button" class="icon-btn h-7 w-[25px]" aria-label=format!("Edit group {}", group.name) disabled=move || busy.get() on:click=move |_| editing.set(Some(Some(edit_group.clone())))>
                                <Icon name="pencil" size=13 />
                            </button>
                            <button type="button" class="icon-btn h-7 w-[25px]" aria-label=format!("Remove group {}", group.name) disabled=move || busy.get() on:click=move |_| change(false, Some(remove_id.clone()))>
                                <Icon name="x" size=13 />
                            </button>
                        </div>
                    }
                } />
            </Show>
        </div>
        <Show when=move || !error.get().is_empty()>
            <p class="form-error mx-3 mb-3" role="alert">{move || error.get()}</p>
        </Show>
        {move || editing.get().map(|group| view! {
            <GroupDialog
                source=source.get_untracked()
                project_id=project_id.get_untracked()
                group=group
                on_close=Callback::new(move |_: ()| editing.set(None))
                on_update=on_update
            />
        })}
    }
}

#[component]
pub fn GroupDialog(
    source: Arc<Source>,
    project_id: String,
    group: Option<CanvasGroup>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_update: Callback<Project>,
) -> impl IntoView {
    let editing = group.is_some();
    let group_id = group.as_ref().map(|g| g.id.clone());
    let name = RwSignal::new(group.as_ref().map(|g| g.name.clone()).unwrap_or_default());
    let color = RwSignal::new(group.as_ref().and_then(|g| g.color.clone()).unwrap_or_else(|| DEFAULT_GROUP_COLOR.into()));
    let members = RwSignal::new(group.as_ref().map(|g| g.node_ids.clone()).unwrap_or_default());
    let query = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let entities: Vec<(String, String, String)> = source.graph.entities.iter().map(|e| (e.id.clone(), e.name.clone(), e.namespace.clone())).collect();
    let source_id = source.id.clone();
    let filtered = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        entities
            .iter()
            .filter(|(_, name, namespace)| format!("{namespace} {name}").to_lowercase().contains(&q))
            .cloned()
            .collect::<Vec<_>>()
    });
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (project_id, source_id, group_id) = (project_id.clone(), source_id.clone(), group_id.clone());
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let result = api::set_group(
                &project_id,
                &source_id,
                group_id.as_deref(),
                &name.get_untracked(),
                &color.get_untracked(),
                &members.get_untracked(),
            )
            .await;
            match result {
                Ok(project) => {
                    on_update.run(project);
                    on_close.run(());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    view! {
        <Modal
            title=if editing { "Edit group" } else { "Create group" }
            subtitle="Choose a color and the nodes that belong together."
            on_close=on_close
            busy=busy
        >
            <form on:submit=submit>
                <label class="form-label" for="group-name">"Group name"</label>
                <input id="group-name" class="field mb-[21px]" bind:value=name required maxlength="80" placeholder="e.g. Customer domain" />
                <label class="form-label" for="group-color">"Group color"</label>
                <div class="mt-2.5 mb-6 flex items-center gap-2.5">
                    {PALETTE.iter().map(|(label, value)| {
                        let value: &'static str = value;
                        view! {
                            <button
                                type="button"
                                class="size-[25px] rounded-full border-[3px] border-surface p-0 outline outline-[#363d44] transition-[outline-color] aria-pressed:outline-2 aria-pressed:outline-[#dde3e8]"
                                style:background=value
                                aria-label=format!("{label} group color")
                                aria-pressed=move || (color.get() == value).to_string()
                                on:click=move |_| color.set(value.into())
                            ></button>
                        }
                    }).collect_view()}
                    <input id="group-color" type="color" class="ml-1.5 h-8 w-[34px] cursor-pointer p-0.5" bind:value=color aria-label="Custom group color" />
                    <span class="font-mono text-[11px] text-muted">{move || color.get().to_uppercase()}</span>
                </div>
                <fieldset class="min-w-0 rounded-lg border border-line-strong p-3.5">
                    <legend class="px-1.5 text-xs text-[#c9d0d6]">
                        "Members " <span class="ml-2 text-[#8f98a1]">{move || format!("{} selected", members.with(|m| m.len()))}</span>
                    </legend>
                    <input type="search" class="field border-line-strong py-2.5 text-[#d0d6dd]" aria-label="Find group members" bind:value=query placeholder="Find a table or endpoint…" />
                    <div class="mt-2 max-h-60 overflow-y-auto">
                        <Show when=move || filtered.with(|f| !f.is_empty()) fallback=|| view! { <p class="form-hint">"No matching nodes."</p> }>
                            <For each=move || filtered.get() key=|(id, _, _)| id.clone() children=move |(id, entity_name, namespace)| {
                                let checked_id = id.clone();
                                view! {
                                    <label class="flex cursor-pointer items-center gap-3 border-b border-line px-1 py-2 last:border-b-0">
                                        <input
                                            type="checkbox"
                                            class="m-0 size-[15px] flex-none"
                                            prop:checked=move || members.with(|m| m.contains(&checked_id))
                                            on:change=move |_| members.update(|m| {
                                                if let Some(index) = m.iter().position(|x| *x == id) { m.remove(index); } else { m.push(id.clone()); }
                                            })
                                        />
                                        <span class="text-xs text-[#d0d6dd] [overflow-wrap:anywhere]">
                                            {entity_name}
                                            <small class="mt-[3px] block font-mono text-[10px] text-[#8c96a0]">{namespace}</small>
                                        </span>
                                    </label>
                                }
                            } />
                        </Show>
                    </div>
                </fieldset>
                <FormError error=error />
                <div class="modal-footer">
                    <span class="form-hint">"Drag the group title to move its nodes together."</span>
                    <button type="submit" class="btn btn-primary" disabled=move || busy.get() || members.with(|m| m.is_empty())>
                        {move || if busy.get() { "Saving…" } else if editing { "Save group" } else { "Create group" }}
                    </button>
                </div>
            </form>
        </Modal>
    }
}
