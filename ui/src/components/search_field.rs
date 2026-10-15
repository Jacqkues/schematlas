//! One search box for the whole app. Focus is drawn on the box itself, inside its own border,
//! rather than as an outline around the bare input, so it never spills out of its rounded shape.
use super::icons::Icon;
use leptos::prelude::*;

#[component]
pub fn SearchField(
    value: RwSignal<String>,
    /// Accessible name of the input.
    label: &'static str,
    placeholder: &'static str,
    /// Width and placement classes for the box.
    #[prop(optional)]
    class: &'static str,
    /// Runs after each keystroke.
    #[prop(optional, into)]
    on_input: Option<Callback<String>>,
    /// Content anchored to the box, such as a results popover.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let input = NodeRef::<leptos::html::Input>::new();
    let clear = move || {
        value.set(String::new());
        if let Some(on_input) = on_input {
            on_input.run(String::new());
        }
        if let Some(el) = input.get_untracked() {
            let _ = el.focus();
        }
    };
    view! {
        <search class=format!("search-field relative flex h-9 items-center gap-2 rounded-lg border border-transparent bg-surface-3 pr-1 pl-3 text-muted transition-[border-color,box-shadow] duration-150 focus-within:border-accent focus-within:text-soft focus-within:ring-2 focus-within:ring-accent/25 hover:border-line {class}")>
            <Icon name="search" size=14 class="shrink-0" />
            <input
                node_ref=input
                class="h-full min-w-0 flex-1 border-0 bg-transparent p-0 text-xs text-ink outline-none placeholder:text-muted focus:outline-none focus-visible:outline-none [&::-webkit-search-cancel-button]:hidden"
                type="search"
                aria-label=label
                placeholder=placeholder
                autocomplete="off"
                spellcheck="false"
                prop:value=move || value.get()
                on:input=move |ev| {
                    let text = event_target_value(&ev);
                    value.set(text.clone());
                    if let Some(on_input) = on_input {
                        on_input.run(text);
                    }
                }
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Escape" && !value.with_untracked(String::is_empty) {
                        ev.prevent_default();
                        ev.stop_propagation();
                        clear();
                    }
                }
            />
            <Show when=move || !value.with(String::is_empty)>
                <button type="button" class="flex size-6 shrink-0 items-center justify-center rounded-md text-muted hover:bg-surface-4 hover:text-ink" aria-label="Clear search" title="Clear (Esc)" on:click=move |_| clear()>
                    <Icon name="x" size=13 />
                </button>
            </Show>
            {children.map(|c| c())}
        </search>
    }
}
