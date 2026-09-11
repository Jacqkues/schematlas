//! Welcome screen shown when a project has no selected source.
use super::icons::Icon;
use leptos::prelude::*;

#[component]
pub fn EmptyState(
    has_project: bool,
    #[prop(optional, into)] name: String,
    #[prop(into)] on_create: Callback<()>,
    #[prop(into)] on_connect: Callback<()>,
    #[prop(into)] on_import: Callback<()>,
    #[prop(into)] on_demo: Callback<()>,
    #[prop(into)] busy: Signal<bool>,
) -> impl IntoView {
    let card = move |title: &'static str,
                     text: &'static str,
                     tags: &'static str,
                     api: bool,
                     on_click: Callback<()>| {
        view! {
            <button
                type="button"
                class="rounded-[9px] border border-line-soft bg-surface p-[23px] px-[25px] text-left transition-[transform,border-color,background-color] duration-200 hover:-translate-y-[3px] hover:border-muted hover:bg-surface-2 max-[1000px]:p-[18px] [@media(max-height:800px)]:px-[22px] [@media(max-height:800px)]:py-[18px]"
                on:click=move |_| on_click.run(())
            >
                <div class="flex items-center justify-between text-soft">
                    <div class="tile"><Icon name=if api { "braces" } else { "database" } size=22 /></div>
                    <Icon name="arrow-up-right" size=18 />
                </div>
                <h2 class="mt-[23px] mb-2.5 text-[17px] font-[580] tracking-[-0.4px] [@media(max-height:800px)]:mt-4">{title}</h2>
                <p class="max-w-[270px] text-xs leading-[1.75] text-muted">{text}</p>
                <span class="mt-6 block border-t border-line-soft pt-3.5 font-mono text-[9px] tracking-[-0.2px] text-muted">{tags}</span>
            </button>
        }
    };
    view! {
        <div class="relative m-auto w-full max-w-[1120px] animate-rise-in px-[70px] pt-[60px] pb-[38px] max-[1180px]:p-10 [@media(max-height:800px)]:pt-[30px] [@media(max-height:800px)]:pb-5">
            <div class="mb-[25px] flex items-center gap-2.5 font-mono text-[10px] tracking-[1.5px] text-muted [@media(max-height:800px)]:mb-[18px]">
                <span class="h-px w-[26px] bg-accent"></span> " A CLEARER VIEW OF YOUR SYSTEM"
            </div>
            <h1 class="max-w-[850px] text-[clamp(34px,3.7vw,58px)] leading-[1.1] font-[480] tracking-[-2.9px] text-ink [overflow-wrap:anywhere] max-[1000px]:text-[40px] [@media(max-height:800px)]:text-[42px]">
                {if has_project {
                    view! { {name.clone()} <span class="block text-muted">"starts here."</span> }.into_any()
                } else {
                    view! { "Complex systems." <span class="block text-muted">"Clear connections."</span> }.into_any()
                }}
            </h1>
            <p class="mt-[22px] mb-[25px] text-sm leading-[1.9] text-soft [@media(max-height:800px)]:mt-[17px] [@media(max-height:800px)]:mb-5 [@media(max-height:800px)]:text-xs">
                "Bring your databases and APIs into one local workspace." <br /> "See the structure. Follow the relationships. Find your bearings."
            </p>
            <Show when=move || !has_project>
                <button type="button" class="btn btn-primary btn-large" on:click=move |_| on_create.run(())>
                    <Icon name="plus" size=17 /> " Create your first project " <Icon name="arrow-right" size=17 class="ml-[18px]" />
                </button>
            </Show>
            <div class="mt-[42px] grid max-w-[750px] grid-cols-2 gap-[18px] max-[1000px]:gap-3 [@media(max-height:800px)]:mt-[25px]">
                {card(
                    "Map your database",
                    "Tables, columns, keys, and the relationships that connect them.",
                    "PostgreSQL · MySQL · SQLite · more",
                    false,
                    if has_project { on_connect } else { on_create },
                )}
                {card(
                    "Explore your API",
                    "Turn an OpenAPI definition into a connected map of endpoints and models.",
                    "OpenAPI 3.x · Swagger 2.0",
                    true,
                    if has_project { on_import } else { on_create },
                )}
            </div>
            <button
                type="button"
                class="mt-[25px] flex items-center gap-[9px] py-1 text-[11px] text-soft transition-colors hover:text-accent"
                disabled=move || busy.get()
                on:click=move |_| on_demo.run(())
            >
                <Icon name="workflow" size=16 />
                {move || if busy.get() { "Creating example…" } else { "Take a look around with an example project" }}
                <Icon name="arrow-right" size=15 />
            </button>
            <div class="mt-[52px] font-mono text-[8px] tracking-[1.3px] text-muted [@media(max-height:800px)]:mt-7">
                "SCHEMATLAS " <span class="mx-3">" / "</span> " UNDERSTAND WHAT CONNECTS."
            </div>
        </div>
    }
}
