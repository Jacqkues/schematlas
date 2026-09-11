//! Project picker, source explorer and the agent toggle.
use super::icons::Icon;
use crate::state::Workspace;
use leptos::prelude::*;

#[component]
pub fn Sidebar(
    #[prop(into)] on_agent: Callback<()>,
    #[prop(into)] on_create: Callback<()>,
    #[prop(into)] on_connect: Callback<()>,
    #[prop(into)] on_import: Callback<()>,
    #[prop(into)] on_edit: Callback<()>,
    #[prop(into)] on_home: Callback<()>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let appearance = expect_context::<crate::appearance::Appearance>();
    let has_project = move || workspace.project.with(|p| p.is_some());
    let sources = move |kind: &'static str| {
        let api = kind == "openapi";
        let list = move || {
            workspace
                .project
                .with(|p| {
                    p.as_ref().map(|p| {
                        p.sources
                            .iter()
                            .filter(|s| s.kind == kind)
                            .map(|s| (s.id.clone(), s.name.clone(), s.graph.entities.len()))
                            .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_default()
        };
        view! {
            <div class=format!(
                "mr-1.5 mb-[5px] ml-2.5 flex items-center justify-between text-[9px] font-[650] tracking-[1px] text-muted {}",
                if api { "mt-[25px]" } else { "" }
            )>
                <span class="flex items-center gap-[7px]">
                    <Icon name=if api { "braces" } else { "database" } size=13 />
                    {if api { " API DEFINITIONS" } else { " DATABASES" }}
                </span>
                <button
                    type="button"
                    class="icon-btn"
                    aria-label=if api { "Import OpenAPI" } else { "Connect database" }
                    on:click=move |_| if api { on_import.run(()) } else { on_connect.run(()) }
                >
                    <Icon name="plus" size=14 />
                </button>
            </div>
            <Show
                when=move || !list().is_empty()
                fallback=move || view! {
                    <p class="mx-3 my-2.5 text-[11px] leading-relaxed text-muted">
                        {if api { "No definitions imported" } else { "No databases connected" }}
                    </p>
                }
            >
                <For each=list key=|(id, _, _)| id.clone() children=move |(id, name, count)| {
                    let active_id = id.clone();
                    let active = Memo::new(move |_| workspace.source_id.get().as_deref() == Some(active_id.as_str()));
                    view! {
                        <button
                            type="button"
                            class="my-[3px] flex w-full items-center gap-2.5 rounded-md border px-3 py-[11px] text-left text-xs transition-colors"
                            class=("border-accent-line", move || active.get())
                            class=("bg-surface-4", move || active.get())
                            class=("text-ink", move || active.get())
                            class=("shadow-[inset_2px_0_var(--color-accent)]", move || active.get())
                            class=("[&>svg]:text-sage", move || active.get())
                            class=("border-transparent", move || !active.get())
                            class=("text-text", move || !active.get())
                            class=("hover:bg-surface", move || !active.get())
                            on:click=move |_| workspace.select_source(id.clone())
                        >
                            <Icon name=if api { "braces" } else { "database" } size=16 />
                            <span class="truncate">{name}</span>
                            <small class="ml-auto font-mono text-[10px] text-muted">{count}</small>
                        </button>
                    }
                } />
            </Show>
        }
    };
    view! {
        <aside class="flex w-[250px] shrink-0 flex-col border-r border-line bg-sidebar px-4 pt-7 max-[1180px]:w-[218px] max-[1180px]:px-3">
            <button type="button" class="px-[9px] text-left" on:click=move |_| on_home.run(()) aria-label="Schematlas projects">
                <div class="flex items-center gap-2.5 text-[22px] font-bold tracking-[-1px]">
                    <img src="/atlas.svg" alt="" width="32" height="32" class="rounded-lg opacity-90" />
                    <span>"schem" <span class="font-normal">"atlas"</span></span>
                </div>
            </button>
            <div class="mx-2.5 mt-[37px] mb-3 flex justify-between text-[9px] font-bold tracking-[1.5px] text-muted">
                "LOCAL WORKSPACE " <span class="font-mono">"01"</span>
            </div>
            <div class="relative flex items-center gap-[9px] rounded-[7px] border border-line-strong bg-surface-3 px-2.5 py-[11px] text-text">
                <Icon name="folder-open" size=16 class="text-sage" />
                <select
                    class="w-full min-w-0 cursor-pointer appearance-none border-0 bg-transparent pr-3 text-xs font-semibold text-ink"
                    aria-label="Current project"
                    on:change=move |ev| {
                        let value = event_target_value(&ev);
                        workspace.select_project(if value.is_empty() { None } else { Some(value) });
                    }
                >
                    <option value="" disabled prop:selected=move || workspace.project_id.get().is_none()>"Select a project"</option>
                    <For
                        each=move || workspace.projects.with(|list| list.iter().map(|p| (p.id.clone(), p.name.clone())).collect::<Vec<_>>())
                        key=|(id, _)| id.clone()
                        children=move |(id, name)| {
                            let selected_id = id.clone();
                            view! {
                                <option value=id prop:selected=move || workspace.project_id.get().as_deref() == Some(selected_id.as_str())>{name}</option>
                            }
                        }
                    />
                </select>
                <Icon name="chevron-down" size=14 class="pointer-events-none absolute right-2.5" />
            </div>
            <button
                type="button"
                class="flex w-full items-center gap-2 px-2.5 py-[13px] text-left text-xs text-muted transition-colors hover:text-accent"
                on:click=move |_| on_create.run(())
            >
                <Icon name="plus" size=15 /> " New project " <kbd class="ml-auto text-[13px]">"＋"</kbd>
            </button>
            <div class="mx-2 mt-2.5 mb-[23px] h-px bg-line-soft"></div>
            <div class="section-heading mx-2.5 mb-[15px]">
                "EXPLORER "
                <Show when=has_project>
                    <button type="button" class="icon-btn" aria-label="Edit project" on:click=move |_| on_edit.run(())>
                        <Icon name="pencil" size=13 />
                    </button>
                </Show>
            </div>
            <Show
                when=has_project
                fallback=|| view! {
                    <p class="mx-2.5 my-0.5 text-xs leading-relaxed text-muted">"Create a project to organize your databases and APIs."</p>
                }
            >
                {sources("database")}
                {sources("openapi")}
            </Show>
            <div class="mt-auto pt-[30px]">
                <Show when=has_project>
                    <button type="button" class="btn mb-[18px] w-full justify-start px-3 py-[11px]" on:click=move |_| on_agent.run(())>
                        <span class="text-[19px] text-sage">"⌘"</span> " Local agent "
                        <small class="ml-auto font-mono text-[10px] text-soft">"ACP"</small>
                    </button>
                </Show>
                <div class="flex gap-2.5 px-2.5 pt-3.5 pb-[23px] text-soft">
                    <Icon name="hard-drive" size=17 />
                    <div>
                        <strong class="text-[11px] font-medium">"Made to stay local."</strong>
                        <p class="mt-[3px] text-[10px] leading-relaxed text-muted">"Your workspace stays on this computer."</p>
                    </div>
                </div>
                <div class="flex h-11 items-center justify-between border-t border-line-soft text-[9px] text-muted">
                    <span class="flex items-center gap-1.5"><span class="status-dot"></span> " Local workspace"</span>
                    <button type="button" class="icon-btn" aria-label=move || if appearance.0.get() { "Switch to dark theme" } else { "Switch to light theme" } on:click=move |_| appearance.toggle()>
                        {move || if appearance.0.get() { "☾" } else { "☀" }}
                    </button>
                </div>
            </div>
        </aside>
    }
}
