//! The home page: every project at a glance, and adding a repository in one step.
use super::icons::Icon;
use super::search_field::SearchField;
use crate::api;
use crate::state::Workspace;
use crate::types::Project;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::sync::Arc;

/// The last path segment, which names a repository's folder.
pub fn folder_name(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
}

/// "today", "yesterday", "5 days ago", or a date, from an RFC 3339 timestamp.
fn updated(iso: &str) -> String {
    let then = js_sys::Date::parse(iso);
    if then.is_nan() {
        return String::new();
    }
    let days = ((js_sys::Date::now() - then) / 86_400_000.0).floor();
    match days as i64 {
        i64::MIN..=0 => "Updated today".into(),
        1 => "Updated yesterday".into(),
        n @ 2..=30 => format!("Updated {n} days ago"),
        _ => {
            let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(then));
            format!(
                "Updated {}",
                String::from(
                    date.to_locale_date_string("default", &wasm_bindgen::JsValue::UNDEFINED)
                )
            )
        }
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Pick a folder and open it: an existing project for that folder, or a new one named after it.
pub fn add_repository(workspace: Workspace, busy: RwSignal<bool>) {
    spawn_local(async move {
        busy.set(true);
        match api::open_dialog("Add a repository", true, None).await {
            Ok(Some(path)) => {
                if let Err(e) = workspace.add_repository(&path).await {
                    workspace.fail(e);
                }
            }
            Ok(None) => {}
            Err(e) => workspace.fail(e),
        }
        busy.set(false);
    });
}

#[component]
fn ProjectCard(project: Arc<Project>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let id = project.id.clone();
    let databases = project.sources.iter().filter(|s| s.is_database()).count();
    let apis = project.sources.len() - databases;
    let repo = project.working_directory.clone();
    let mut facts = vec![];
    if databases > 0 {
        facts.push(("database", plural(databases, "database", "databases")));
    }
    if apis > 0 {
        facts.push(("braces", plural(apis, "API", "APIs")));
    }
    view! {
        <li>
            <button
                type="button"
                class="group flex h-full w-full flex-col gap-3 rounded-2xl border border-line bg-surface p-5 text-left transition-colors hover:border-line-strong hover:bg-surface-3"
                on:click=move |_| workspace.select_project(Some(id.clone()))
            >
                <span class="flex items-center gap-3">
                    <span class="flex size-9 shrink-0 items-center justify-center rounded-xl bg-surface-3 text-sage group-hover:bg-surface-4"><Icon name="folder-git" size=18 /></span>
                    <span class="min-w-0 flex-1">
                        <strong class="block truncate text-[14px] font-medium text-ink">{project.name.clone()}</strong>
                        {match repo.clone() {
                            Some(path) => view! { <span class="block truncate font-mono text-[11px] text-muted" title=path.clone()>{folder_name(&path).to_string()}</span> }.into_any(),
                            None => view! { <span class="block text-[11px] text-faint">"No repository yet"</span> }.into_any(),
                        }}
                    </span>
                    <Icon name="arrow-right" size=15 class="text-faint opacity-0 transition-opacity group-hover:opacity-100" />
                </span>
                {(!project.description.trim().is_empty()).then(|| view! {
                    <span class="line-clamp-2 text-xs leading-5 text-soft">{project.description.clone()}</span>
                })}
                <span class="mt-auto flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted">
                    {facts.into_iter().map(|(icon, text)| view! { <span class="flex items-center gap-1.5"><Icon name=icon size=12 />{text}</span> }).collect_view()}
                    <span class="ml-auto text-faint">{updated(&project.updated_at)}</span>
                </span>
            </button>
        </li>
    }
}

#[component]
pub fn ProjectsHome(
    #[prop(into)] on_empty: Callback<()>,
    #[prop(into)] on_demo: Callback<()>,
    #[prop(into)] demo_busy: Signal<bool>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let desktop = api::desktop();
    let busy = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let any = move || workspace.projects.with(|list| !list.is_empty());
    let shown = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        workspace.projects.with(|list| {
            let mut list: Vec<Arc<Project>> = list
                .iter()
                .filter(|p| {
                    q.is_empty()
                        || p.name.to_lowercase().contains(&q)
                        || p.working_directory
                            .as_deref()
                            .is_some_and(|d| d.to_lowercase().contains(&q))
                })
                .cloned()
                .collect();
            list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            list
        })
    });
    let add = move |_| {
        if desktop {
            add_repository(workspace, busy);
        }
    };
    let add_button = move |class: &'static str| {
        view! {
            <button
                type="button"
                class=format!("btn btn-primary {class}")
                disabled=move || busy.get() || !desktop
                title=if desktop { "Choose a folder on this computer" } else { "Repositories open in the desktop app" }
                on:click=add
            >
                <Icon name="folder-plus" size=16 />
                {move || if busy.get() { "Opening…" } else { "Add repository" }}
            </button>
        }
    };
    view! {
        <div class="flex min-h-0 flex-1 flex-col overflow-auto">
            <Show when=any fallback=move || view! {
                <section class="m-auto w-full max-w-[640px] animate-rise-in px-10 py-16 text-center">
                    <div class="mx-auto mb-7 flex size-14 items-center justify-center rounded-2xl bg-surface-3 text-sage"><Icon name="folder-git" size=26 /></div>
                    <h1 class="text-[clamp(28px,3vw,40px)] leading-tight font-medium tracking-[-1.4px]">"Make sense of your codebase."</h1>
                    <p class="mx-auto mt-4 max-w-[470px] text-sm leading-7 text-muted">"Add a repository. Your coding agent maps it, explains its flows and keeps the docs next to the code."</p>
                    <div class="mt-8 flex flex-col items-center gap-3">
                        {add_button("px-5 py-3")}
                        <Show when=move || !desktop>
                            <p class="text-[11px] text-faint">"Browser preview · repositories open in the desktop app."</p>
                        </Show>
                        <button type="button" class="text-xs text-muted hover:text-ink" on:click=move |_| on_empty.run(())>"Or start an empty project"</button>
                    </div>
                    <button type="button" class="mt-12 inline-flex items-center gap-2 text-xs text-muted hover:text-ink" aria-label="Take a look around with an example project" disabled=move || demo_busy.get() on:click=move |_| on_demo.run(())>{move || if demo_busy.get() { "Opening example…" } else { "Explore an example project" }}<Icon name="arrow-right" size=14 /></button>
                </section>
            }>
                <div class="mx-auto w-full max-w-[1120px] px-8 py-8">
                    <header class="mb-6 flex flex-wrap items-center gap-3">
                        <div class="mr-auto">
                            <h1 class="text-xl font-medium tracking-tight">"Projects"</h1>
                            <p class="mt-1 text-xs text-muted">"One project per repository. Its diagrams and docs live in the repository itself."</p>
                        </div>
                        <SearchField value=query label="Search projects" placeholder="Find a project…" class="w-[240px]" />
                        <button type="button" class="btn bg-transparent" on:click=move |_| on_empty.run(())><Icon name="plus" size=15 /> "Empty project"</button>
                        {add_button("")}
                    </header>
                    <Show when=move || !desktop>
                        <p class="mb-4 text-[11px] text-faint">"Browser preview · adding a repository needs the desktop app."</p>
                    </Show>
                    <ul class="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-3" aria-label="Projects">
                        <For each=move || shown.get() key=|p| (p.id.clone(), p.updated_at.clone(), p.name.clone(), p.working_directory.clone()) children=|project| view! { <ProjectCard project=project /> } />
                        <Show when=move || desktop && query.with(|q| q.trim().is_empty())>
                            <li>
                                <button type="button" class="flex h-full min-h-[132px] w-full flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-line text-muted transition-colors hover:border-line-strong hover:text-ink" disabled=move || busy.get() on:click=add>
                                    <Icon name="folder-plus" size=20 />
                                    <span class="text-xs">"Add a repository"</span>
                                </button>
                            </li>
                        </Show>
                    </ul>
                    <Show when=move || shown.with(Vec::is_empty) && !query.with(|q| q.trim().is_empty())>
                        <p class="py-10 text-center text-xs text-muted">"No project matches."</p>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn folder_names() {
        assert_eq!(super::folder_name("/Users/a/Code/shop/"), "shop");
        assert_eq!(super::folder_name("C:\\code\\shop"), "shop");
        assert_eq!(super::folder_name("shop"), "shop");
    }
}
