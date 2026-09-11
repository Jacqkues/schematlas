//! One source: toolbar, graph canvas, map options popover and inspector.
use super::graph::canvas::GraphCanvas;
use super::groups::CanvasGroups;
use super::icons::Icon;
use super::inspector::Inspector;
use super::schema_filter::SchemaFilter;
use crate::layout::schema_namespaces;
use crate::state::Workspace;
use crate::types::{database_name, Entity, Position, Source};
use leptos::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Groups,
    Filters,
    Details,
}

impl Panel {
    fn title(self) -> &'static str {
        match self {
            Panel::Groups => "Groups",
            Panel::Filters => "Schemas",
            Panel::Details => "Source details",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Panel::Groups => "Manage groups",
            Panel::Filters => "Schema filters",
            Panel::Details => "Source details",
        }
    }
}

const NAV_BUTTON: &str = "flex items-center gap-1.5 rounded-md border border-transparent px-[9px] py-2 text-[11px] text-text transition-colors hover:border-accent-line hover:bg-accent-soft hover:text-accent-text aria-expanded:border-accent-line aria-expanded:bg-accent-soft aria-expanded:text-accent-text";

fn locale_date(iso: &str) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(iso));
    String::from(date.to_locale_string("default", &wasm_bindgen::JsValue::UNDEFINED))
}

#[component]
pub fn SourceWorkspace(
    #[prop(into)] on_api: Callback<()>,
    #[prop(into)] on_refresh: Callback<()>,
    #[prop(into)] on_reconnect: Callback<()>,
    #[prop(into)] on_export: Callback<()>,
    #[prop(into)] on_remove: Callback<()>,
    #[prop(into)] on_save: Callback<HashMap<String, Position>>,
    #[prop(into)] refreshing: Signal<bool>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let source: Signal<Arc<Source>> = Signal::derive(move || {
        workspace
            .source
            .get()
            .unwrap_or_else(|| Arc::new(Source::default()))
    });
    let project_id = Signal::derive(move || {
        workspace
            .project
            .get()
            .map(|p| p.id.clone())
            .unwrap_or_default()
    });
    let project_name = Signal::derive(move || {
        workspace
            .project
            .get()
            .map(|p| p.name.clone())
            .unwrap_or_default()
    });
    let query = RwSignal::new(String::new());
    let namespaces = RwSignal::new(Vec::<String>::new());
    let related = RwSignal::new(true);
    let selected = RwSignal::new(None::<Entity>);
    let panel = RwSignal::new(None::<Panel>);
    let focus_node_ids = RwSignal::new(Vec::<String>::new());
    let namespace_counts = Memo::new(move |_| source.with(|s| schema_namespaces(&s.graph)));
    let is_api = Signal::derive(move || source.with(|s| !s.is_database()));
    let toggle_panel = move |next: Panel| {
        selected.set(None);
        panel.update(|p| *p = if *p == Some(next) { None } else { Some(next) });
        if next == Panel::Filters {
            focus_node_ids.set(vec![]);
        }
    };
    let focus_group = Callback::new(move |ids: Vec<String>| {
        query.set(String::new());
        namespaces.set(vec![]);
        focus_node_ids.set(ids);
        panel.set(None);
    });
    let on_update = Callback::new(move |project| workspace.upsert(project, false));
    let expanded = move |which: Panel| move || (panel.get() == Some(which)).to_string();
    view! {
        <div class="flex h-full min-h-0 animate-fade-in flex-col">
            <header class="flex min-h-[58px] shrink-0 items-center gap-[18px] border-b border-line bg-surface px-[18px] max-[1100px]:gap-2 max-[1100px]:px-3">
                <div class="flex min-w-0 flex-1 items-center gap-[9px] text-sage">
                    {move || if is_api.get() { view! { <Icon name="braces" size=17 /> } } else { view! { <Icon name="database" size=17 /> } }}
                    <h1 class="truncate text-sm font-semibold text-ink">{move || source.with(|s| s.name.clone())}</h1>
                </div>
                <search class="flex w-[clamp(140px,20vw,260px)] items-center gap-2 text-faint max-[1100px]:w-40">
                    <Icon name="search" size=14 />
                    <input
                        class="w-full border-0 bg-transparent py-[9px] text-xs text-ink"
                        type="search"
                        aria-label="Search schema"
                        placeholder="Find a table…"
                        bind:value=query
                        on:input=move |_| focus_node_ids.set(vec![])
                    />
                </search>
                <nav class="flex gap-[5px]" aria-label="Map options">
                    <button type="button" class=NAV_BUTTON aria-expanded=expanded(Panel::Filters) aria-controls="map-options" on:click=move |_| toggle_panel(Panel::Filters)>
                        <Icon name="layers-3" size=15 /> "Schemas"
                        <Show when=move || !namespaces.with(|n| n.is_empty())>
                            <span class="text-[10px] text-accent-muted">{move || namespaces.with(|n| n.len())}</span>
                        </Show>
                    </button>
                    <button type="button" class=NAV_BUTTON aria-expanded=expanded(Panel::Groups) aria-controls="map-options" on:click=move |_| toggle_panel(Panel::Groups)>
                        <Icon name="group" size=15 /> "Groups"
                    </button>
                    <button type="button" class=NAV_BUTTON aria-label="Source details and actions" aria-expanded=expanded(Panel::Details) aria-controls="map-options" on:click=move |_| toggle_panel(Panel::Details)>
                        <Icon name="ellipsis" size=18 />
                    </button>
                </nav>
            </header>
            <div class="relative flex min-h-0 flex-1">
                <GraphCanvas
                    source=source
                    query=query
                    namespaces=namespaces
                    related=related
                    focus_node_ids=focus_node_ids
                    on_select=Callback::new(move |entity: Option<Entity>| {
                        selected.set(entity);
                        panel.set(None);
                    })
                    on_save=on_save
                />
                {move || panel.get().map(|which| view! {
                    <aside
                        id="map-options"
                        class="absolute top-[52px] right-3 z-20 max-h-[calc(100%-68px)] w-[310px] animate-rise-in overflow-auto rounded-[10px] border border-line-strong bg-surface-2 shadow-[0_12px_35px_#0007]"
                        aria-label=which.label()
                    >
                        <header class="flex items-center justify-between border-b border-line px-4 py-3.5">
                            <h2 class="text-[13px] font-bold">{which.title()}</h2>
                            <button type="button" class="icon-btn" aria-label="Close map options" on:click=move |_| panel.set(None)><Icon name="x" size=16 /></button>
                        </header>
                        {match which {
                            Panel::Groups => view! { <CanvasGroups source=source project_id=project_id on_update=on_update on_focus=focus_group /> }.into_any(),
                            Panel::Filters => view! { <SchemaFilter namespaces=namespace_counts selected=namespaces related=related api=is_api /> }.into_any(),
                            Panel::Details => view! {
                                <div class="flex flex-col gap-2.5 p-4 text-xs">
                                    <p class="text-text">
                                        {move || format!("{} / {}", project_name.get(), source.with(|s| s.database_kind.as_deref().map(database_name).unwrap_or("OpenAPI")))}
                                    </p>
                                    <dl class="mt-1.5 mb-3 grid grid-cols-2 gap-2 text-[11px]">
                                        <dt class="text-muted">"Nodes"</dt>
                                        <dd class="text-right">{move || source.with(|s| s.graph.entities.len())}</dd>
                                        <dt class="text-muted">"Relationships"</dt>
                                        <dd class="text-right">{move || source.with(|s| s.graph.relations.len())}</dd>
                                        <dt class="text-muted">"Last inspected"</dt>
                                        <dd class="text-right">{move || source.with(|s| locale_date(&s.imported_at))}</dd>
                                    </dl>
                                    {move || if is_api.get() {
                                        view! {
                                            <button type="button" class="btn justify-start" on:click=move |_| on_api.run(())><Icon name="plug-zap" size=14 /> "API connection"</button>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <button type="button" class="btn justify-start" disabled=move || refreshing.get() on:click=move |_| on_refresh.run(())>
                                                <Icon name="refresh-cw" size=14 />
                                                {move || if refreshing.get() { "Refreshing…" } else { "Refresh schema" }}
                                            </button>
                                            <button type="button" class="btn justify-start" on:click=move |_| on_reconnect.run(())><Icon name="plug-zap" size=14 /> "Reconnect database"</button>
                                        }.into_any()
                                    }}
                                    <button type="button" class="btn justify-start" on:click=move |_| on_export.run(())><Icon name="download" size=14 /> "Export"</button>
                                    <button type="button" class="btn justify-start" on:click=move |_| on_remove.run(())><Icon name="trash-2" size=14 /> "Delete source"</button>
                                    <Show when=move || source.with(|s| !s.graph.warnings.is_empty())>
                                        <details>
                                            <summary>{move || format!("Import notes ({})", source.with(|s| s.graph.warnings.len()))}</summary>
                                            <ul class="list-disc pl-4">
                                                {move || source.with(|s| s.graph.warnings.iter().map(|w| view! { <li class="my-2">{w.clone()}</li> }).collect_view())}
                                            </ul>
                                        </details>
                                    </Show>
                                </div>
                            }.into_any(),
                        }}
                    </aside>
                })}
                {move || selected.get().map(|entity| view! {
                    <Inspector
                        entity=entity
                        source=source.get_untracked()
                        on_close=Callback::new(move |_: ()| selected.set(None))
                        on_select=Callback::new(move |entity: Entity| selected.set(Some(entity)))
                    />
                })}
            </div>
        </div>
    }
}
