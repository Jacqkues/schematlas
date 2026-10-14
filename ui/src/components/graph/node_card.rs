//! One diagram node: a compact box with its label, an optional description excerpt, connection
//! handles on hover, and in-place label editing.
use crate::components::icons::Icon;
use crate::diagram_layout::{card_height, color, detail_lines, note, CARD_WIDTH};
use crate::types::DiagramNode;
use leptos::prelude::*;
use std::sync::Arc;

pub fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "store" | "database" | "db" => "database",
        "service" | "api" | "server" => "box",
        "external" | "third-party" => "arrow-up-right",
        "client" | "ui" | "app" => "layout-grid",
        "queue" | "worker" | "job" => "workflow",
        "actor" | "user" => "users",
        "doc" | "note" => "file-text",
        "decision" => "spline",
        "class" | "entity" | "table" => "table-2",
        _ => "code",
    }
}

/// Plain text of the first lines of a Markdown body, for the card excerpt.
fn excerpt(body: &str) -> String {
    body.lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', '>']).trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .replace(['*', '`', '_'], "")
        .chars()
        .take(160)
        .collect()
}

/// (id, client x, client y, pointer id, shift held)
pub type PointerArgs = (String, f64, f64, i32, bool);

const HANDLE: &str = "absolute z-[2] size-3 rounded-full border-2 border-accent bg-surface opacity-0 transition-opacity duration-100 group-hover:opacity-100 group-data-selected:opacity-100 hover:scale-125 hover:bg-accent cursor-crosshair";

#[component]
pub fn DiagramNodeCard(
    node: Memo<Arc<DiagramNode>>,
    position: Memo<(f64, f64)>,
    selected: Memo<bool>,
    dimmed: Memo<bool>,
    editing: Memo<bool>,
    comments: Memo<usize>,
    /// The card is a connection target under the pointer.
    target: Memo<bool>,
    #[prop(into)] on_pointer_down: Callback<PointerArgs>,
    #[prop(into)] on_handle_down: Callback<PointerArgs>,
    #[prop(into)] on_edit: Callback<String>,
    /// (id, client x, client y)
    #[prop(into)]
    on_menu: Callback<(String, f64, f64)>,
    /// (id, new label); None cancels.
    #[prop(into)]
    on_rename: Callback<(String, Option<String>)>,
    #[prop(into)] on_select: Callback<String>,
) -> impl IntoView {
    let id = move || node.with(|n| n.id.clone());
    let accent = move || node.with(|n| color(n));
    let input = NodeRef::<leptos::html::Textarea>::new();
    // Enter, Escape and the blur that follows the editor's removal must report only once.
    let finished = StoredValue::new(false);
    Effect::new(move |_| {
        if editing.get() {
            finished.set_value(false);
            if let Some(el) = input.get() {
                // Focus at once so no keystroke is lost; select again once the value is in place
                // (unless typing already started) so typing replaces the whole label.
                // The value is set here rather than left to the renderer, which may apply it
                // after this effect and leave the caret after an unselected label.
                el.set_value(&node.with_untracked(|n| n.label.clone()));
                let _ = el.focus();
                el.select();
                let initial = el.value();
                request_animation_frame(move || {
                    if el.value() == initial {
                        let _ = el.focus();
                        el.select();
                    }
                });
            }
        }
    });
    let finish = move |save: bool| {
        if finished.get_value() {
            return;
        }
        finished.set_value(true);
        let value = input.get_untracked().map(|el| el.value());
        on_rename.run((id(), if save { value } else { None }));
    };
    let handle = move |side: &'static str| {
        let class = match side {
            "top" => format!("{HANDLE} -top-1.5 left-1/2 -translate-x-1/2"),
            "bottom" => format!("{HANDLE} -bottom-1.5 left-1/2 -translate-x-1/2"),
            "left" => format!("{HANDLE} top-1/2 -left-1.5 -translate-y-1/2"),
            _ => format!("{HANDLE} top-1/2 -right-1.5 -translate-y-1/2"),
        };
        view! {
            <span
                class=class
                title="Drag to connect, or drop on empty canvas to create a connected node"
                aria-hidden="true"
                data-handle=side
                on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                    if ev.button() != 0 { return; }
                    ev.stop_propagation();
                    ev.prevent_default();
                    on_handle_down.run((id(), ev.client_x() as f64, ev.client_y() as f64, ev.pointer_id(), false));
                }
            ></span>
        }
    };
    view! {
        <div
            class="absolute top-0 left-0"
            class=("z-10", move || selected.get() || editing.get())
            style:transform=move || {
                let (x, y) = position.get();
                format!("translate({x}px, {y}px)")
            }
            style:width=format!("{CARD_WIDTH}px")
            on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                if ev.button() != 0 || editing.get_untracked() {
                    return;
                }
                ev.stop_propagation();
                on_pointer_down.run((id(), ev.client_x() as f64, ev.client_y() as f64, ev.pointer_id(), ev.shift_key()));
            }
            on:dblclick=move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                on_edit.run(id());
            }
            on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                ev.prevent_default();
                ev.stop_propagation();
                on_menu.run((id(), ev.client_x() as f64, ev.client_y() as f64));
            }
        >
            <article
                class="diagram-node group relative flex cursor-default flex-col rounded-[10px] border-[1.5px] bg-surface-2 shadow-[0_1px_2px_#0003,0_4px_14px_#0002] transition-[box-shadow,border-color,opacity] duration-150"
                class=("border-line-strong", move || !selected.get() && !target.get() && accent().is_none())
                class=("ring-2", move || selected.get() || target.get())
                class=("ring-accent", move || selected.get() || target.get())
                class=("ring-offset-2", move || selected.get())
                class=("ring-offset-canvas", move || selected.get())
                class=("opacity-35", move || dimmed.get())
                style:border-color=move || accent()
                style:height=move || format!("{}px", node.with(|n| card_height(n)))
                tabindex="0"
                role="button"
                aria-label=move || node.with(|n| format!("{}, {}", n.label, n.kind()))
                aria-pressed=move || selected.get().to_string()
                data-node-id=id
                data-selected=move || selected.get().then_some("")
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if editing.get_untracked() || !matches!(ev.key().as_str(), "Enter" | " ") {
                        return;
                    }
                    ev.prevent_default();
                    ev.stop_propagation();
                    if selected.get_untracked() && ev.key() == "Enter" {
                        on_edit.run(id());
                    } else {
                        on_select.run(id());
                    }
                }
            >
                {handle("top")}
                {handle("right")}
                {handle("bottom")}
                {handle("left")}
                <div class="flex h-16 shrink-0 items-center gap-2.5 px-3.5">
                    <span
                        class="flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-4 text-sage"
                        style:color=move || accent()
                        title=move || node.with(|n| n.kind().to_owned())
                    >
                        {move || view! { <Icon name=kind_icon(&node.with(|n| n.kind().to_owned())) size=15 /> }}
                    </span>
                    {move || if editing.get() {
                        view! {
                            <textarea
                                node_ref=input
                                class="h-11 min-w-0 flex-1 resize-none rounded border border-accent-line bg-field px-1.5 py-1 text-[13px] leading-[1.3] font-semibold text-ink outline-none"
                                aria-label="Node label"
                                on:pointerdown=|ev| ev.stop_propagation()
                                on:dblclick=|ev| ev.stop_propagation()
                                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                    ev.stop_propagation();
                                    match ev.key().as_str() {
                                        "Enter" if !ev.shift_key() => { ev.prevent_default(); finish(true); }
                                        "Escape" => { ev.prevent_default(); finish(false); }
                                        _ => {}
                                    }
                                }
                                on:blur=move |_| finish(true)
                            ></textarea>
                        }.into_any()
                    } else {
                        view! {
                            <strong class="line-clamp-2 min-w-0 flex-1 text-[13px] leading-[1.3] font-semibold tracking-[-0.1px] text-ink [overflow-wrap:anywhere]" title=move || node.with(|n| n.label.clone())>
                                {move || node.with(|n| n.label.clone())}
                            </strong>
                        }.into_any()
                    }}
                </div>
                <Show when=move || node.with(|n| note(n).is_some())>
                    <p class="-mt-2 line-clamp-2 px-3.5 text-[11px] leading-[1.45] text-muted">
                        {move || node.with(|n| note(n).map(excerpt).unwrap_or_default())}
                    </p>
                </Show>
                <Show when=move || node.with(|n| !n.details.is_empty())>
                    <ul class="mx-2 mb-2 flex flex-col overflow-hidden rounded-md border border-line-soft bg-surface-3 py-[3px] font-mono text-[10.5px] text-soft" aria-label="Members">
                        {move || node.with(|n| detail_lines(n)).into_iter().map(|line| view! {
                            <li class="h-[18px] truncate px-2 leading-[18px]" title=line.clone()>{line.clone()}</li>
                        }).collect_view()}
                    </ul>
                </Show>
                <div class="pointer-events-none absolute -top-2.5 right-2 flex gap-1">
                    <Show when=move || node.with(|n| !n.annotation.code.is_empty())>
                        <span class="flex items-center gap-0.5 rounded-full border border-line bg-surface-3 px-1.5 py-px font-mono text-[9px] text-soft" title="Code references">
                            <Icon name="code" size=9 /> {move || node.with(|n| n.annotation.code.len())}
                        </span>
                    </Show>
                    <Show when=move || { comments.get() > 0 }>
                        <span class="flex items-center gap-0.5 rounded-full bg-accent px-1.5 py-px text-[9px] font-semibold text-accent-ink" title="Team comments">
                            <Icon name="message-square" size=9 /> {move || comments.get()}
                        </span>
                    </Show>
                </div>
            </article>
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn excerpts_are_plain_text() {
        assert_eq!(
            super::excerpt("# Title\n- **Bold** `code`\n\n"),
            "Title Bold code"
        );
    }
}
