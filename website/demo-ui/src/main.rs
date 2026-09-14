//! Public, in-memory demo shell around Schematlas's actual Leptos graph.
//! No Tauri bridge, storage, credentials, database client or agent runtime is linked.
mod components;
mod layout_worker;
use components::{graph::canvas::GraphCanvas, inspector::Inspector};
use leptos::prelude::*;
use schematlas_ui::{layout, relationships, smart_layout, types};
use std::{collections::HashMap, sync::Arc};
use types::{Entity, Position, Project, Source};

// Only the worker's JS error conversion is needed from the desktop boundary.
mod api {
    pub fn js_error(value: wasm_bindgen::JsValue) -> String {
        value
            .as_string()
            .unwrap_or_else(|| "Layout unavailable. Please try again.".into())
    }
}

fn samples() -> Vec<Source> {
    let mut project: Project = serde_json::from_str(include_str!("sample.json"))
        .expect("The bundled sample must be valid");
    for source in &mut project.sources {
        if source.positions.is_empty() {
            source.positions = smart_layout::smart_layout(&source.graph, source.groups());
        }
    }
    project.sources
}

#[component]
fn Demo() -> impl IntoView {
    let sources = RwSignal::new(samples());
    let active = RwSignal::new(0_usize);
    let revision = RwSignal::new(0_u32);
    let query = RwSignal::new(String::new());
    let selected = RwSignal::new(None::<Entity>);
    let source = Signal::derive(move || Arc::new(sources.with(|s| s[active.get()].clone())));
    let choose = move |index| {
        active.set(index);
        query.set(String::new());
        selected.set(None);
    };
    let save = Callback::new(move |positions: HashMap<String, Position>| {
        sources.update(|s| s[active.get_untracked()].positions = positions);
    });
    view! {
        <main class="demo-shell">
            <header class="demo-header">
                <div class="demo-identity"><span class="demo-mark">"s"</span><div><strong>"Northstar Commerce"</strong><span>"Sample workspace"</span></div></div>
                <nav class="demo-tabs" aria-label="Sample source">
                    <button type="button" aria-pressed=move || (active.get() == 0).to_string() on:click=move |_| choose(0)>"Database"</button>
                    <button type="button" aria-pressed=move || (active.get() == 1).to_string() on:click=move |_| choose(1)>"OpenAPI"</button>
                </nav>
                <button type="button" class="demo-reset" on:click=move |_| {
                    sources.set(samples());
                    query.set(String::new());
                    selected.set(None);
                    revision.update(|r| *r += 1);
                }>"Reset demo"</button>
            </header>
            <div class="demo-toolbar">
                <span class="demo-count">{move || source.with(|s| format!("{} nodes · {} relationships", s.graph.entities.len(), s.graph.relations.len()))}</span>
                <input type="search" aria-label="Search sample schema" placeholder="Find a node…" bind:value=query />
                <select aria-label="Inspect a node" prop:value=move || selected.with(|e| e.as_ref().map(|e| e.id.clone()).unwrap_or_default()) on:change=move |ev| {
                    let id = event_target_value(&ev);
                    selected.set(source.with_untracked(|s| s.graph.entities.iter().find(|e| e.id == id).cloned()));
                }>
                    <option value="">"Inspect a node…"</option>
                    {move || source.with(|s| s.graph.entities.iter().map(|e| view! {
                        <option value=e.id.clone()>{format!("{}{}", e.method.as_ref().map(|m| format!("{m} ")).unwrap_or_default(), e.name)}</option>
                    }).collect_view())}
                </select>
            </div>
            <div class="demo-workspace">
                // Remount on source/reset so each map gets a fresh viewport and selection.
                <For each=move || vec![(active.get(), revision.get())] key=|key| *key children=move |_| view! {
                    <GraphCanvas source=source query=query namespaces=Signal::derive(Vec::<String>::new)
                        related=Signal::derive(|| true) focus_node_ids=Signal::derive(Vec::<String>::new)
                        on_select=Callback::new(move |entity| selected.set(entity)) on_save=save />
                } />
                {move || selected.get().map(|entity| view! {
                    <Inspector entity=entity source=source.get_untracked()
                        on_close=Callback::new(move |()| selected.set(None))
                        on_select=Callback::new(move |entity| selected.set(Some(entity))) />
                })}
            </div>
            <footer class="demo-footer"><span>"Drag to explore · Scroll to zoom · Select, then Inspect"</span><span>"Sample data only · Changes stay in this demo"</span></footer>
        </main>
    }
}

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(Demo);
}
