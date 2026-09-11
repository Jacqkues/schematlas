//! Namespace chips and the linked-schemas toggle shown in the map options popover.
use super::icons::Icon;
use leptos::prelude::*;

const CHIP: &str = "flex items-center gap-[9px] rounded-[5px] border border-line-soft bg-surface px-[9px] py-[5px] font-mono text-[10px] whitespace-nowrap text-[#babcbf] transition-colors aria-pressed:border-[#4a565d] aria-pressed:bg-[#20272b] aria-pressed:text-[#e0e7eb]";

#[component]
pub fn SchemaFilter(
    #[prop(into)] namespaces: Signal<Vec<(String, usize)>>,
    selected: RwSignal<Vec<String>>,
    related: RwSignal<bool>,
    #[prop(into)] api: Signal<bool>,
) -> impl IntoView {
    let toggle = move |name: String| {
        selected.update(|list| {
            if let Some(index) = list.iter().position(|n| *n == name) {
                list.remove(index);
            } else {
                list.push(name);
            }
        });
    };
    view! {
        <div class="flex flex-wrap items-center gap-3.5 p-4" aria-label=move || if api.get() { "Filter API groups" } else { "Filter database schemas" }>
            <div class="flex flex-1 flex-wrap gap-1.5 p-0.5">
                <button
                    type="button"
                    class=CHIP
                    aria-pressed=move || selected.with(|s| s.is_empty()).to_string()
                    on:click=move |_| selected.set(vec![])
                >
                    "All " <span class="text-[8px] text-[#abadb0]">{move || namespaces.with(|n| n.len())}</span>
                </button>
                <For each=move || namespaces.get() key=|(name, _)| name.clone() children=move |(name, count)| {
                    let pressed_name = name.clone();
                    let toggled_name = name.clone();
                    view! {
                        <button
                            type="button"
                            class=CHIP
                            aria-pressed=move || selected.with(|s| s.contains(&pressed_name)).to_string()
                            on:click=move |_| toggle(toggled_name.clone())
                        >
                            {name}
                            <span class="text-[8px] text-[#abadb0]">{count}</span>
                        </button>
                    }
                } />
            </div>
            <label class="flex items-center gap-1.5 text-[10px] whitespace-nowrap text-[#a1adb6]">
                <input type="checkbox" class="accent-accent" bind:checked=related />
                <Icon name="link-2" size=13 />
                {move || if api.get() { " Include linked groups" } else { " Include linked schemas" }}
            </label>
        </div>
    }
}
