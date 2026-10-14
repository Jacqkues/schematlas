//! Shared single-choice picker. The trigger keeps focus while the listbox is open.
use super::icons::Icon;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn Select(
    #[prop(into)] value: Signal<String>,
    #[prop(into)] options: Signal<Vec<(String, String)>>,
    #[prop(into)] on_change: Callback<String>,
    #[prop(default = "")] id: &'static str,
    #[prop(default = "")] label: &'static str,
    #[prop(default = "")] class: &'static str,
    #[prop(default = false)] compact: bool,
) -> impl IntoView {
    let root = NodeRef::<leptos::html::Div>::new();
    let menu = NodeRef::<leptos::html::Div>::new();
    let trigger = NodeRef::<leptos::html::Button>::new();
    let open = RwSignal::new(false);
    let active = RwSignal::new(0_usize);
    let position = RwSignal::new(String::new());
    let prefix = StoredValue::new(String::new());
    let typed_at = StoredValue::new(0_f64);
    let list_id = StoredValue::new(format!("choices-{}", uuid::Uuid::new_v4()));
    let selected = move || {
        options.with(|options| {
            options
                .iter()
                .position(|(id, _)| *id == value.get())
                .unwrap_or(0)
        })
    };
    let place = move || {
        if let Some(button) = trigger.get_untracked() {
            let rect = button.get_bounding_client_rect();
            let window = web_sys::window();
            let height = window
                .as_ref()
                .and_then(|w| w.inner_height().ok())
                .and_then(|v| v.as_f64())
                .unwrap_or(800.0);
            let width = window
                .as_ref()
                .and_then(|w| w.inner_width().ok())
                .and_then(|v| v.as_f64())
                .unwrap_or(1200.0);
            let below = height - rect.bottom() - 12.0;
            let above = rect.top() - 12.0;
            let upwards = below < 240.0 && above > below;
            let available = if upwards { above } else { below }.clamp(40.0, 280.0);
            let menu_width = rect.width().max(200.0).min(width - 24.0);
            let left = rect.left().min(width - menu_width - 12.0).max(12.0);
            let vertical = if upwards {
                format!("top:auto;bottom:{}px", height - rect.top() + 6.0)
            } else {
                format!("bottom:auto;top:{}px", rect.bottom() + 6.0)
            };
            position.set(format!(
                "left:{left}px;width:{menu_width}px;max-height:{available}px;{vertical}"
            ));
        }
    };
    let reveal = move || {
        active.set(selected());
        place();
        open.set(true);
    };
    let close = move || open.set(false);
    let choose = move |index: usize| {
        if let Some((id, _)) = options.with_untracked(|o| o.get(index).cloned()) {
            close();
            if let Some(button) = trigger.get_untracked() {
                let _ = button.focus();
            }
            on_change.run(id);
        }
    };
    let outside = move |target: Option<web_sys::EventTarget>| {
        if open.get_untracked()
            && !target
                .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
                .is_some_and(|target| {
                    root.get_untracked()
                        .is_some_and(|root| root.contains(Some(&target)))
                })
        {
            close();
        }
    };
    let pointer = window_event_listener(leptos::ev::pointerdown, move |ev| outside(ev.target()));
    let focus = window_event_listener(leptos::ev::focusin, move |ev| outside(ev.target()));
    let resize = window_event_listener(leptos::ev::resize, move |_| close());
    on_cleanup(move || {
        pointer.remove();
        focus.remove();
        resize.remove();
    });
    let keydown = move |ev: leptos::ev::KeyboardEvent| {
        let count = options.with_untracked(Vec::len);
        if count == 0 {
            return;
        }
        match ev.key().as_str() {
            "ArrowDown" | "ArrowUp" => {
                ev.prevent_default();
                if !open.get_untracked() {
                    reveal();
                } else if ev.key() == "ArrowDown" {
                    active.update(|v| *v = (*v + 1).min(count - 1));
                } else {
                    active.update(|v| *v = v.saturating_sub(1));
                }
            }
            "Home" | "End" => {
                ev.prevent_default();
                if !open.get_untracked() {
                    reveal();
                }
                active.set(if ev.key() == "Home" { 0 } else { count - 1 });
            }
            "Enter" | " " => {
                ev.prevent_default();
                if open.get_untracked() {
                    choose(active.get_untracked());
                } else {
                    reveal();
                }
            }
            "Escape" if open.get_untracked() => {
                ev.prevent_default();
                ev.stop_propagation();
                close();
            }
            "Tab" => close(),
            key if key.chars().count() == 1
                && !ev.ctrl_key()
                && !ev.meta_key()
                && !ev.alt_key() =>
            {
                ev.prevent_default();
                let now = crate::api::now_ms();
                if now - typed_at.get_value() > 700.0 {
                    prefix.set_value(String::new());
                }
                typed_at.set_value(now);
                prefix.update_value(|v| v.push_str(&key.to_lowercase()));
                let text = prefix.get_value();
                if !open.get_untracked() {
                    reveal();
                }
                if let Some(index) = options.with_untracked(|o| {
                    o.iter()
                        .position(|(_, name)| name.to_lowercase().starts_with(&text))
                }) {
                    active.set(index);
                }
            }
            _ => {}
        }
    };
    Effect::new(move |_| {
        if open.get() {
            if let Some(element) = menu.get() {
                // Top-layer rendering avoids clipping by a scrolling native dialog.
                // Older WebViews use the fixed-position fallback instead.
                if element.show_popover().is_err() {
                    let _ = element.remove_attribute("popover");
                }
            }
        }
    });
    Effect::new(move |_| {
        if open.get() {
            let index = active.get();
            request_animation_frame(move || {
                if let Some(element) = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.get_element_by_id(&format!("{}-{index}", list_id.get_value())))
                {
                    if let Some(menu) = element.parent_element() {
                        let item = element.get_bounding_client_rect();
                        let bounds = menu.get_bounding_client_rect();
                        let delta = if item.top() < bounds.top() {
                            item.top() - bounds.top()
                        } else if item.bottom() > bounds.bottom() {
                            item.bottom() - bounds.bottom()
                        } else {
                            0.0
                        };
                        menu.set_scroll_top(menu.scroll_top() + delta as i32);
                    }
                }
            });
        }
    });
    let entries = move || options.get().into_iter().enumerate().collect::<Vec<_>>();
    view! {
        <div node_ref=root class=format!("min-w-0 {class}")>
            <button node_ref=trigger id=id type="button" role="combobox" aria-label=label aria-haspopup="listbox" aria-expanded=move || open.get().to_string() aria-controls=list_id.get_value() aria-activedescendant=move || open.get().then(|| format!("{}-{}", list_id.get_value(), active.get()))
                class=if compact { "select-trigger min-h-8 border-transparent bg-transparent px-2 py-1.5" } else { "select-trigger min-h-11 border-line bg-field px-3 py-2.5" }
                disabled=move || options.with(Vec::is_empty) on:keydown=keydown on:click=move |_| { if open.get_untracked() { close(); } else { reveal(); } }>
                <span class="min-w-0 flex-1 truncate text-left">{move || options.with(|o| o.iter().find(|(id, _)| *id == value.get()).map(|(_, name)| name.clone()).unwrap_or_else(|| "Choose…".into()))}</span>
                <Icon name="chevron-down" size=14 class="shrink-0 text-muted" />
            </button>
            <Show when=move || open.get()>
                <div node_ref=menu popover="manual" id=list_id.get_value() role="listbox" aria-label=format!("{label} options") class="fixed right-auto m-0 z-[100] overflow-auto overscroll-contain rounded-xl border border-line-strong bg-surface p-1.5 text-ink shadow-[0_8px_32px_#0004]" style=move || position.get()>
                    <For each=entries key=|(_, (id, _))| id.clone() children=move |(index, (id, name))| {
                        let is_selected = Memo::new(move |_| value.get() == id);
                        view! { <button id=format!("{}-{index}", list_id.get_value()) type="button" role="option" tabindex="-1" aria-selected=move || is_selected.get().to_string() data-active=move || (active.get() == index).then_some("") class="select-option" on:pointermove=move |_| active.set(index) on:pointerdown=move |ev| ev.prevent_default() on:click=move |_| choose(index)>
                            <span class="min-w-0 flex-1 [overflow-wrap:anywhere]">{name}</span>
                            <span class="size-4 shrink-0"><Show when=move || is_selected.get()><Icon name="check" size=14 /></Show></span>
                        </button> }
                    } />
                </div>
            </Show>
        </div>
    }
}
