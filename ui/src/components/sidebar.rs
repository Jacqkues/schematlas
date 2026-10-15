//! Project switcher, file explorer, and workspace utilities.
use super::explorer::ProjectExplorer;
use super::icons::Icon;
use crate::state::Workspace;
use leptos::prelude::*;

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
    #[prop(into)] on_edit: Callback<()>,
    #[prop(into)] on_home: Callback<()>,
    /// Back to the list of every project.
    #[prop(into)]
    on_projects: Callback<()>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let appearance = expect_context::<crate::appearance::Appearance>();
    let has_project = move || workspace.project.with(|p| p.is_some());
    view! {
        <aside class="flex shrink-0 flex-col border-r border-line-soft bg-sidebar p-3" class=("w-[264px]", move || !collapsed.get()) class=("w-[60px]", move || collapsed.get()) aria-label="Workspace navigation">
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
                    <ProjectExplorer on_new_diagram=on_new_diagram on_new_doc=on_new_doc on_connect=on_connect on_import=on_import />
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
