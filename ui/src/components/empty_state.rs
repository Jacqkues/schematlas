//! A quiet starting point for the documentation workspace.
use super::icons::Icon;
use leptos::prelude::*;

#[component]
pub fn EmptyState(
    #[prop(into)] on_create: Callback<()>,
    #[prop(into)] on_demo: Callback<()>,
    #[prop(into)] busy: Signal<bool>,
) -> impl IntoView {
    view! {
        <div class="flex min-h-0 flex-1 flex-col overflow-auto">
            <header class="flex h-16 shrink-0 items-center px-7 text-sm text-muted">"Your workspace"</header>
            <section class="m-auto w-full max-w-[760px] animate-rise-in px-10 py-16 text-center">
                <div class="mx-auto mb-7 flex size-14 items-center justify-center rounded-2xl bg-surface-3 text-sage"><Icon name="workflow" size=26 /></div>
                <h1 class="text-[clamp(28px,3vw,40px)] leading-tight font-medium tracking-[-1.4px]">"Make sense of your codebase."</h1>
                <p class="mx-auto mt-4 max-w-[470px] text-sm leading-7 text-muted">"Turn your code into maps and explanations with the coding agent you already use."</p>
                <button type="button" class="btn btn-primary mt-8 px-5 py-3" on:click=move |_| on_create.run(())><Icon name="plus" size=17 /> "Create your first project"</button>
                <div class="mt-12 grid grid-cols-3 gap-3 text-left max-[800px]:grid-cols-1">
                    {[("folder-git", "Bring your code", "Open a local repository."), ("sparkles", "Ask your agent", "Map a system or explain a flow."), ("file-text", "Keep the knowledge", "Docs live alongside your code.")].into_iter().map(|(icon, title, body)| view! {
                        <div class="rounded-2xl border border-line-soft p-5"><Icon name=icon size=18 class="mb-4 text-muted" /><h2 class="text-xs font-medium">{title}</h2><p class="mt-2 text-xs leading-5 text-muted">{body}</p></div>
                    }).collect_view()}
                </div>
                <button type="button" class="mt-8 inline-flex items-center gap-2 text-xs text-muted hover:text-ink" aria-label="Take a look around with an example project" disabled=move || busy.get() on:click=move |_| on_demo.run(())>{move || if busy.get() { "Opening example…" } else { "Explore an example project" }}<Icon name="arrow-right" size=14 /></button>
            </section>
            <p class="px-6 pb-5 text-center text-[11px] text-faint">"Open source. Your code, your agent, your workspace."</p>
        </div>
    }
}
