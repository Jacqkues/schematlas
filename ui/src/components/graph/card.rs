//! One table card on the canvas, with its ports and the inspect action.
use super::geometry::OVERVIEW_ZOOM;
use crate::components::icons::Icon;
use crate::layout::MAX_FIELDS;
use crate::types::Entity;
use leptos::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardState {
    Idle,
    Selected,
    Related,
    Dimmed,
}

/// Which columns carry outgoing (foreign key) and incoming (referenced) ports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ports {
    pub outgoing: HashSet<String>,
    pub incoming: HashSet<String>,
}

const FIELD_PORT: &str = "absolute top-1/2 size-[5px] -translate-y-1/2 rounded-full border border-surface bg-soft opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-data-selected:opacity-100";

#[component]
pub fn EntityCard(
    entity: Memo<Arc<Entity>>,
    position: Memo<(f64, f64)>,
    state: Memo<CardState>,
    ports: Memo<Arc<Ports>>,
    #[prop(into)] zoom: Signal<f64>,
    /// (id, client x, client y, pointer id)
    #[prop(into)]
    on_pointer_down: Callback<(String, f64, f64, i32)>,
    #[prop(into)] on_inspect: Callback<Arc<Entity>>,
    #[prop(into)] on_select: Callback<String>,
    /// (id, dx, dy) in canvas units.
    #[prop(into)]
    on_nudge: Callback<(String, f64, f64)>,
    #[prop(into)] on_clear: Callback<()>,
) -> impl IntoView {
    let selected = move || state.get() == CardState::Selected;
    let detailed = Memo::new(move |_| zoom.get() >= OVERVIEW_ZOOM || selected());
    let kind = move || entity.with(|e| e.kind.clone());
    let id = move || entity.with(|e| e.id.clone());
    // Screen readers get the same reference the rest of the app uses, plus the shape of the table.
    let label = move || {
        entity.with(|e| {
            let count = e.fields.len();
            let unit = if matches!(e.kind.as_str(), "table" | "view") {
                "column"
            } else {
                "field"
            };
            let plural = if count == 1 { "" } else { "s" };
            match &e.method {
                Some(method) => format!("{method} {}, endpoint, {count} {unit}{plural}", e.name),
                None => format!(
                    "{}.{}, {}, {count} {unit}{plural}",
                    e.namespace, e.name, e.kind
                ),
            }
        })
    };
    let keys = move |ev: leptos::ev::KeyboardEvent| {
        let step = if ev.shift_key() { 50.0 } else { 10.0 };
        let delta = match ev.key().as_str() {
            "ArrowLeft" => Some((-step, 0.0)),
            "ArrowRight" => Some((step, 0.0)),
            "ArrowUp" => Some((0.0, -step)),
            "ArrowDown" => Some((0.0, step)),
            _ => None,
        };
        if let Some((dx, dy)) = delta {
            ev.prevent_default();
            ev.stop_propagation();
            on_nudge.run((id(), dx, dy));
            return;
        }
        match ev.key().as_str() {
            // A selected card is already the inspector's subject, so Enter opens it.
            "Enter" | " " => {
                ev.prevent_default();
                ev.stop_propagation();
                if selected() {
                    on_inspect.run(entity.get());
                } else {
                    on_select.run(id());
                }
            }
            "Escape" if selected() => {
                ev.stop_propagation();
                on_clear.run(());
            }
            _ => {}
        }
    };
    view! {
        <div
            class="absolute top-0 left-0"
            class=("z-10", selected)
            style:transform=move || {
                let (x, y) = position.get();
                format!("translate({x}px, {y}px)")
            }
            on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                if ev.button() != 0 {
                    return;
                }
                ev.stop_propagation();
                on_pointer_down.run((id(), ev.client_x() as f64, ev.client_y() as f64, ev.pointer_id()));
            }
        >
            <Show when=selected>
                <button
                    type="button"
                    class="absolute -top-[46px] left-1/2 flex -translate-x-1/2 items-center gap-[7px] rounded-lg border border-accent-line bg-accent-soft px-3 py-2 text-xs whitespace-nowrap text-accent-text shadow-[0_4px_12px_#0005] transition-colors hover:bg-accent-soft"
                    aria-label=move || entity.with(|e| format!("Inspect {}.{}", e.namespace, e.name))
                    on:pointerdown=move |ev: leptos::ev::PointerEvent| { if ev.button() != 1 { ev.stop_propagation(); } }
                    on:click=move |ev| {
                        ev.stop_propagation();
                        on_inspect.run(entity.get());
                    }
                >
                    <Icon name="eye" size=16 /> <span>"Inspect"</span>
                </button>
            </Show>
            <article
                class="entity-node group cursor-grab active:cursor-grabbing"
                tabindex="0"
                role="button"
                aria-label=label
                aria-pressed=move || selected().to_string()
                title="Enter or Space selects. Arrow keys move 10px; Shift moves 50px. Escape clears the selection."
                on:keydown=keys
                data-state=move || match state.get() {
                    CardState::Dimmed => "dimmed",
                    CardState::Related | CardState::Selected => "related",
                    CardState::Idle => "idle",
                }
                data-selected=move || selected().then_some("")
            >
                <span class="absolute top-[31px] -left-[3.5px] size-[7px] rounded-full border-2 border-surface bg-muted"></span>
                <span class="absolute top-[31px] -right-[3.5px] size-[7px] rounded-full border-2 border-surface bg-muted"></span>
                <div class="h-16 rounded-t-[7px] border-b border-line-soft bg-node-header px-3.5 pt-3.5 pb-2.5">
                    <div class="entity-title flex items-center gap-2 text-ink">
                        {move || {
                            let e = entity.get();
                            match (&e.method, e.kind.as_str()) {
                                (Some(method), _) => view! {
                                    <span
                                        class="rounded-[3px] bg-surface px-[5px] py-[3px] font-mono text-[8px] font-bold text-soft"
                                        class=("bg-danger-soft", method == "DELETE")
                                        class=("text-[#edb1ac]", method == "DELETE")
                                    >{method.clone()}</span>
                                }.into_any(),
                                (None, "schema") => view! { <Icon name="braces" size=17 /> }.into_any(),
                                (None, "view") => view! { <Icon name="eye" size=17 /> }.into_any(),
                                _ => view! { <Icon name="table-2" size=17 /> }.into_any(),
                            }
                        }}
                        <strong
                            class="truncate font-mono font-[650] tracking-[-0.25px] text-ink"
                            class=("text-[11px]", move || kind() == "operation")
                            class=("text-sm", move || kind() != "operation")
                        >{move || entity.with(|e| e.name.clone())}</strong>
                    </div>
                    <span class="mt-[7px] ml-[25px] flex justify-between font-mono text-[9px] text-faint">
                        {move || entity.with(|e| e.namespace.clone())}
                        <span class="text-[7px] tracking-[0.9px]">{move || if kind() == "operation" { "ENDPOINT".to_string() } else { kind().to_uppercase() }}</span>
                    </span>
                </div>
                <div
                    class="py-1.5 text-text [contain:layout_style]"
                    class=("bg-[repeating-linear-gradient(transparent_0_28px,#75838d12_28px_29px)]", move || !detailed.get())
                >
                    <For
                        each=move || entity.with(|e| e.fields.iter().take(MAX_FIELDS).cloned().collect::<Vec<_>>())
                        key=|f| f.name.clone()
                        children=move |field| {
                            let name = field.name.clone();
                            let incoming_name = name.clone();
                            let outgoing_name = name.clone();
                            let symbol_name = name.clone();
                            let outgoing = Memo::new(move |_| ports.with(|p| p.outgoing.contains(&outgoing_name)));
                            let incoming = Memo::new(move |_| ports.with(|p| p.incoming.contains(&incoming_name)));
                            let primary = field.primary_key;
                            let data_type = if field.data_type.is_empty() { "any".to_string() } else { field.data_type.clone() };
                            let _ = symbol_name;
                            view! {
                                <div class="relative flex h-[29px] items-center gap-[7px] px-3 font-mono text-xs transition-colors hover:bg-surface-3">
                                    // A connected column can anchor on either side as cards move.
                                    <Show when=move || incoming.get() || outgoing.get()>
                                        <span class=format!("{FIELD_PORT} -left-[2.5px]")></span>
                                    </Show>
                                    <Show when=move || detailed.get()>
                                        <span class="flex w-[13px] shrink-0 items-center justify-center text-soft">
                                            {move || if primary {
                                                view! { <Icon name="key-round" size=12 /> }.into_any()
                                            } else if outgoing.get() {
                                                view! { <Icon name="link-2" size=12 /> }.into_any()
                                            } else {
                                                view! { <span class="size-[3px] rounded-full bg-[#7c7e81]"></span> }.into_any()
                                            }}
                                        </span>
                                        <span class="truncate">{name.clone()}</span>
                                        <span class="ml-auto max-w-[105px] truncate text-[10px] text-faint">{data_type.clone()}</span>
                                    </Show>
                                    <Show when=move || incoming.get() || outgoing.get()>
                                        <span class=format!("{FIELD_PORT} -right-[2.5px]")></span>
                                    </Show>
                                </div>
                            }
                        }
                    />
                </div>
                <Show when=move || entity.with(|e| e.fields.len() > MAX_FIELDS)>
                    <div class="h-[29px] bg-surface px-[13px] py-[7px] text-[9px] text-text">
                        {move || format!("+ {} more · inspect for details", entity.with(|e| e.fields.len() - MAX_FIELDS))}
                    </div>
                </Show>
                <div class="flex h-7 justify-between rounded-b-[7px] border-t border-line-soft px-[13px] py-2 font-mono text-[9px] text-faint">
                    <span class="flex items-center gap-1">
                        {move || entity.with(|e| format!("{} {}", e.fields.len(), if e.kind == "table" || e.kind == "view" { "columns" } else { "fields" }))}
                    </span>
                    <Show when=move || entity.with(|e| e.fields.iter().any(|f| f.primary_key))>
                        <span class="flex items-center gap-1"><Icon name="key-round" size=10 /> " primary key"</span>
                    </Show>
                </div>
            </article>
        </div>
    }
}
