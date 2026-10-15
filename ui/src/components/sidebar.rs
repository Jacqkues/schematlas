//! Project switcher, file explorer, and workspace utilities.
use super::explorer::ProjectExplorer;
use super::icons::Icon;
use crate::state::Workspace;
use leptos::prelude::*;

const WIDTH_KEY: &str = "atlas:sidebar-width";
const DEFAULT_WIDTH: f64 = 264.0;
const MIN_WIDTH: f64 = 200.0;
const MAX_WIDTH: f64 = 520.0;

#[component]
pub fn Sidebar(
    collapsed: RwSignal<bool>,
    agent_open: RwSignal<bool>,
    #[prop(into)] on_agent: Callback<()>,
    #[prop(into)] on_team: Callback<()>,
    #[prop(into)] on_new_diagram: Callback<()>,
    #[prop(into)] on_new_doc: Callback<()>,
    #[prop(into)] on_connect: Callback<()>,
    #[prop(into)] on_import: Callback<()>,
    #[prop(into)] on_paste: Callback<()>,
    #[prop(into)] on_edit: Callback<()>,
    #[prop(into)] on_home: Callback<()>,
    /// Back to the list of every project.
    #[prop(into)]
    on_projects: Callback<()>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let appearance = expect_context::<crate::appearance::Appearance>();
    let has_project = move || workspace.project.with(|p| p.is_some());
    // Drag the right edge to resize; the width is remembered, a double-click restores it.
    let width = RwSignal::new(
        crate::api::local_storage_get(WIDTH_KEY)
            .and_then(|w| w.parse::<f64>().ok())
            .map(|w| w.clamp(MIN_WIDTH, MAX_WIDTH))
            .unwrap_or(DEFAULT_WIDTH),
    );
    let resizing = StoredValue::new(None::<(f64, f64)>);
    let save_width = move || {
        crate::api::local_storage_set(WIDTH_KEY, &format!("{:.0}", width.get_untracked()));
    };
    view! {
        <aside class="relative flex shrink-0 flex-col border-r border-line-soft bg-sidebar p-3" style:width=move || if collapsed.get() { "60px".to_string() } else { format!("{:.0}px", width.get()) } aria-label="Workspace navigation">
            <Show when=move || !collapsed.get()>
                <div
                    class="absolute top-0 right-[-3px] bottom-0 z-20 w-[6px] cursor-col-resize touch-none hover:bg-accent/40 active:bg-accent/60"
                    role="separator"
                    aria-orientation="vertical"
                    aria-label="Resize sidebar"
                    aria-valuemin=MIN_WIDTH
                    aria-valuemax=MAX_WIDTH
                    aria-valuenow=move || format!("{:.0}", width.get())
                    tabindex="0"
                    title="Drag to resize · double-click to reset"
                    on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                        if ev.button() != 0 { return; }
                        ev.prevent_default();
                        if let Some(target) = ev.target().and_then(|t| wasm_bindgen::JsCast::dyn_into::<web_sys::Element>(t).ok()) {
                            let _ = target.set_pointer_capture(ev.pointer_id());
                        }
                        resizing.set_value(Some((ev.client_x() as f64, width.get_untracked())));
                    }
                    on:pointermove=move |ev: leptos::ev::PointerEvent| {
                        if let Some((start, from)) = resizing.get_value() {
                            width.set((from + ev.client_x() as f64 - start).clamp(MIN_WIDTH, MAX_WIDTH));
                        }
                    }
                    on:pointerup=move |_| { if resizing.get_value().is_some() { resizing.set_value(None); save_width(); } }
                    on:pointercancel=move |_| resizing.set_value(None)
                    on:dblclick=move |_| { width.set(DEFAULT_WIDTH); save_width(); }
                    on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                        let step = match ev.key().as_str() { "ArrowLeft" => -16.0, "ArrowRight" => 16.0, _ => return };
                        ev.prevent_default();
                        width.update(|w| *w = (*w + step).clamp(MIN_WIDTH, MAX_WIDTH));
                        save_width();
                    }
                ></div>
            </Show>
            <div class="mb-4 flex h-9 items-center justify-between gap-2">
                <Show when=move || !collapsed.get()>
                    <button type="button" class="flex items-center gap-2.5 px-2 text-[15px] font-semibold tracking-tight" on:click=move |_| on_projects.run(()) aria-label="All projects" title="All projects"><img src="/atlas.svg" alt="" width="25" height="25" class="rounded-lg" /> "Schematlas"</button>
                </Show>
                <button type="button" class="icon-btn" aria-label=move || if collapsed.get() { "Expand sidebar" } else { "Collapse sidebar" } title="Toggle sidebar" aria-expanded=move || !collapsed.get() on:click=move |_| collapsed.update(|v| *v = !*v)><Icon name="panel-left" size=18 /></button>
            </div>
            <Show when=move || collapsed.get()>
                <button type="button" class="icon-btn mb-2" aria-label="All projects" title="All projects" on:click=move |_| on_projects.run(())><Icon name="folder-git" size=18 /></button>
                <Show when=has_project>
                    <button type="button" class="icon-btn mb-2" aria-label="Project overview" title="Project overview" on:click=move |_| on_home.run(())><Icon name="layout-grid" size=18 /></button>
                    <button type="button" class="icon-btn" aria-label="Ask your agent" title="Local agent" aria-pressed=move || agent_open.get() on:click=move |_| on_agent.run(())><Icon name="sparkles" size=18 /></button>
                </Show>
            </Show>
            <Show when=move || !collapsed.get()>
                <Show when=has_project fallback=move || view! {
                    <button type="button" class="flex h-8 items-center gap-2 rounded-md bg-surface-3 px-2 text-xs text-text" aria-current="page"><Icon name="layout-grid" size=15 /> "Projects"</button>
                    <nav class="mt-4 flex min-h-0 flex-1 flex-col gap-0.5 overflow-auto" aria-label="Recent projects">
                        <Show when=move || workspace.projects.with(|l| !l.is_empty())>
                            <span class="mb-1 px-2 text-[11px] text-muted">"Recent"</span>
                        </Show>
                        <For each=move || { let mut list = workspace.projects.get(); list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)); list.truncate(12); list } key=|p| (p.id.clone(), p.name.clone()) children=move |p| {
                            let id = p.id.clone();
                            view! { <button type="button" class="nav-item" on:click=move |_| workspace.select_project(Some(id.clone()))><Icon name="folder-git" size=15 class="text-muted" /><span class="truncate">{p.name.clone()}</span></button> }
                        } />
                    </nav>
                }>
                    <div class="mb-3 flex items-center gap-1 rounded-lg border border-line-soft px-1 py-1.5">
                        <div class="relative flex min-w-0 flex-1 items-center gap-2 pl-2">
                            <Icon name="folder-git" size=16 class="text-muted" />
                            <super::select::Select label="Current project" class="min-w-0 flex-1" compact=true value=Signal::derive(move || workspace.project_id.get().unwrap_or_default()) on_change=Callback::new(move |value: String| workspace.select_project(if value.is_empty() { None } else { Some(value) })) options=Signal::derive(move || workspace.projects.with(|list| list.iter().map(|p| (p.id.clone(), p.name.clone())).collect())) />
                        </div>
                        <button type="button" class="icon-btn size-7" aria-label="All projects" title="All projects" on:click=move |_| on_projects.run(())><Icon name="layout-grid" size=15 /></button>
                        <button type="button" class="icon-btn size-7" aria-label="Edit project" title="Project settings" on:click=move |_| on_edit.run(())><Icon name="ellipsis" size=16 /></button>
                    </div>
                    <button type="button" class="flex h-8 items-center gap-2 rounded-md px-2 text-xs text-text hover:bg-surface-3" class=("bg-surface-3", move || workspace.item.get().is_none() && workspace.source_id.get().is_none()) aria-current=move || (workspace.item.get().is_none() && workspace.source_id.get().is_none()).then_some("page") on:click=move |_| on_home.run(())><Icon name="layout-grid" size=15 /> "Overview"</button>
                    <ProjectExplorer on_new_diagram=on_new_diagram on_new_doc=on_new_doc on_connect=on_connect on_import=on_import on_paste=on_paste />
                </Show>
            </Show>
            <div class="mt-auto border-t border-line-soft pt-3">
                <Show when=move || has_project() && !collapsed.get()>
                    <button type="button" class="nav-item" aria-label="Ask your agent" aria-pressed=move || agent_open.get() on:click=move |_| on_agent.run(())><Icon name="sparkles" size=16 /> "Ask your agent"</button>
                    <button type="button" class="nav-item" on:click=move |_| on_team.run(())><Icon name="users" size=16 /> "Team" <span class="ml-auto text-[11px] text-muted">{move || workspace.sync.with(|s| s.as_ref().filter(|s| s.enabled).map(|s| format!("{} here", s.presence.len())).unwrap_or_default())}</span></button>
                </Show>
                <div class="mt-2 flex items-center justify-between gap-2">
                    <Show when=move || !collapsed.get()><span class="flex items-center gap-2 pl-3 text-[11px] text-muted"><Icon name="hard-drive" size=14 /> "Local workspace"</span></Show>
                    <button type="button" class="icon-btn" aria-label=move || if appearance.0.get() { "Switch to dark theme" } else { "Switch to light theme" } on:click=move |_| appearance.toggle()>{move || if appearance.0.get() { "☾" } else { "☀" }}</button>
                </div>
            </div>
        </aside>
    }
}
