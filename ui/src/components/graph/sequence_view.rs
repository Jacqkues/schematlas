//! A sequence diagram drawn the way readers expect: participants across the top and bottom,
//! lifelines down the page, messages as arrows in time order, notes, activation bars and
//! `loop`/`alt`/`opt` frames. Read-only; participants open their details like nodes elsewhere.
use super::node_card::{kind_icon, LabelText};
use crate::components::icons::Icon;
use crate::types::Diagram;
use leptos::prelude::*;
use schematlas_parser::model::Arrow;
use schematlas_ui::sequence_layout::{self, Message, SequenceLayout, LABEL_MAX_WIDTH};
use std::collections::HashMap;
use std::sync::Arc;

/// A label shortened to the width the layout gave it.
fn fitted(label: &str) -> String {
    let plain = crate::label::plain(label);
    if sequence_layout::text_width(&plain) <= LABEL_MAX_WIDTH {
        return plain;
    }
    let mut out = String::new();
    for c in plain.chars() {
        if sequence_layout::text_width(&out) > LABEL_MAX_WIDTH - 12.0 {
            break;
        }
        out.push(c);
    }
    format!("{}…", out.trim_end())
}

fn message_path(m: &Message) -> String {
    if m.to_self {
        let x = m.x1;
        format!(
            "M{x} {y} L{r} {y} L{r} {b} L{e} {b}",
            y = m.y,
            r = x + 34.0,
            b = m.y + 20.0,
            e = x + 4.0
        )
    } else {
        format!("M{} {y} L{} {y}", m.x1, m.x2, y = m.y)
    }
}

#[component]
pub fn SequenceView(
    #[prop(into)] diagram: Signal<Arc<Diagram>>,
    /// The selected participant; none in an inline preview.
    #[prop(optional)]
    selected: Option<RwSignal<Option<String>>>,
    /// Open a participant's details, on click.
    #[prop(optional, into)]
    on_details: Option<Callback<String>>,
    /// Participants whose cited code changed: `changed`, `broken` or `local`.
    #[prop(optional, into)]
    changes: Option<Signal<HashMap<String, String>>>,
    /// Drawn inside a doc: no scrolling, zoom or selection.
    #[prop(optional)]
    still: bool,
) -> impl IntoView {
    let layout = Memo::new(move |_| diagram.with(|d| Arc::new(sequence_layout::layout(d))));
    let zoom = RwSignal::new(1.0_f64);
    let marker = if still { "seq-still" } else { "seq" };
    let drawing = move || {
        let l: Arc<SequenceLayout> = layout.get();
        let d = diagram.get();
        let lifelines = l
            .participants
            .iter()
            .map(|p| {
                view! {
                    <line x1=p.x y1=l.lifeline_top x2=p.x y2=l.lifeline_bottom class="stroke-line-strong" stroke-width="1.5" stroke-dasharray="4 5"></line>
                }
            })
            .collect_view();
        let frames = l
            .frames
            .iter()
            .map(|f| {
                let tab = f.block.clone();
                let tab_width = sequence_layout::text_width(&tab) * 1.15 + 18.0;
                let branches = f
                    .branches
                    .iter()
                    .map(|(y, label)| {
                        view! {
                            <line x1=f.x y1=*y x2=f.x + f.width y2=*y class="stroke-line-strong" stroke-width="1.2" stroke-dasharray="6 5"></line>
                            {(!label.is_empty()).then(|| view! {
                                <text x=f.x + 12.0 y=y + 16.0 class="fill-muted stroke-canvas text-[11px] italic" stroke-width="4" stroke-linejoin="round" paint-order="stroke">{format!("[{}]", crate::label::plain(label))}</text>
                            })}
                        }
                    })
                    .collect_view();
                view! {
                    <g>
                        <path d=format!("M{x} {y} h{w} v16 l-8 8 h-{w2} z", x = f.x, y = f.y, w = tab_width, w2 = tab_width - 8.0) class="fill-surface-3 stroke-line-strong" stroke-width="1.2"></path>
                        <text x=f.x + 9.0 y=f.y + 16.0 class="fill-ink text-[11px] font-semibold">{tab}</text>
                        {(!f.label.is_empty()).then(|| view! {
                            <text x=f.x + tab_width + 10.0 y=f.y + 16.0 class="fill-muted stroke-canvas text-[11px] italic" stroke-width="4" stroke-linejoin="round" paint-order="stroke">{format!("[{}]", crate::label::plain(&f.label))}</text>
                        })}
                        {branches}
                    </g>
                }
            })
            .collect_view();
        let frame_bodies = l
            .frames
            .iter()
            .map(|f| view! {
                <rect x=f.x y=f.y width=f.width height=f.height rx="8" class="fill-surface/40 stroke-line-strong" stroke-width="1.2"></rect>
            })
            .collect_view();
        let bars = l
            .bars
            .iter()
            .map(|b| {
                view! {
                    <rect x=b.x y=b.y1 width=sequence_layout::BAR_WIDTH height=(b.y2 - b.y1).max(8.0) rx="2" class="fill-surface-3 stroke-line-strong" stroke-width="1.2"></rect>
                }
            })
            .collect_view();
        let messages = l
            .messages
            .iter()
            .map(|m| {
                let end = match m.arrow {
                    Arrow::Filled | Arrow::Both => Some(format!("url(#{marker}-filled)")),
                    Arrow::Async => Some(format!("url(#{marker}-async)")),
                    Arrow::Open | Arrow::Cross => None,
                };
                let start = (m.arrow == Arrow::Both).then(|| format!("url(#{marker}-filled)"));
                let cross = (m.arrow == Arrow::Cross).then(|| {
                    let x = m.x2;
                    view! {
                        <path d=format!("M{} {} l8 8 M{} {} l8 -8", x - 4.0, m.y - 4.0, x - 4.0, m.y + 4.0) class="stroke-diagram-edge" stroke-width="2"></path>
                    }
                });
                let (lx, ly, anchor) = if m.to_self {
                    (m.x1 + 42.0, m.y + 4.0, "start")
                } else {
                    ((m.x1 + m.x2) / 2.0, m.y - 8.0, "middle")
                };
                let number = m.number.map(|n| {
                    view! {
                        <circle cx=m.x1 cy=m.y r="9" class="fill-accent"></circle>
                        <text x=m.x1 y=m.y + 3.5 text-anchor="middle" class="fill-accent-ink text-[9.5px] font-bold">{n}</text>
                    }
                });
                view! {
                    <g class="sequence-message">
                        <path d=message_path(m) class="fill-none stroke-diagram-edge" stroke-width="1.6" stroke-dasharray=m.dotted.then_some("6 5") marker-end=end marker-start=start></path>
                        {cross}
                        {(!m.label.is_empty()).then(|| view! {
                            <text x=lx y=ly text-anchor=anchor class="fill-text stroke-canvas font-mono text-[11.5px]" stroke-width="5" stroke-linejoin="round" paint-order="stroke">
                                <title>{crate::label::plain(&m.label)}</title>
                                {fitted(&m.label)}
                            </text>
                        })}
                        {number}
                    </g>
                }
            })
            .collect_view();
        let notes = l
            .notes
            .iter()
            .map(|n| {
                view! {
                    <div class="absolute overflow-hidden rounded-md border border-[#e0bd8366] bg-[#e0bd831f] px-3 py-2 text-[12px] leading-[17px] text-ink [overflow-wrap:anywhere]" style:left=format!("{}px", n.x) style:top=format!("{}px", n.y) style:width=format!("{}px", n.width) style:height=format!("{}px", n.height)>
                        <LabelText label=n.text.clone() />
                    </div>
                }
            })
            .collect_view();
        let boxes = |y: f64| {
            l.participants
                .iter()
                .filter_map(|p| {
                    let node = d.nodes.iter().find(|n| n.id == p.id)?.clone();
                    let id = node.id.clone();
                    let (select_id, details_id, pressed_id, change_id) =
                        (id.clone(), id.clone(), id.clone(), id.clone());
                    let pressed = move || selected.is_some_and(|s| s.with(|s| s.as_deref() == Some(pressed_id.as_str())));
                    let change = move || changes.and_then(|c| c.with(|c| c.get(&change_id).cloned()));
                    let interactive = !still;
                    Some(view! {
                        <button
                            type="button"
                            class="sequence-participant absolute flex items-center justify-center gap-2 rounded-[10px] border-[1.5px] border-line-strong bg-surface-2 px-3 text-left shadow-[0_1px_2px_#0003] transition-colors aria-pressed:border-accent disabled:cursor-default"
                            data-node-id=id.clone()
                            aria-pressed=move || pressed().to_string()
                            disabled=!interactive
                            style:left=format!("{}px", p.x - p.width / 2.0)
                            style:top=format!("{y}px")
                            style:width=format!("{}px", p.width)
                            style:height=format!("{}px", p.height)
                            on:click=move |_| {
                                if let Some(s) = selected {
                                    s.set(Some(select_id.clone()));
                                }
                                if let Some(open) = on_details {
                                    open.run(details_id.clone());
                                }
                            }
                        >
                            <span class="flex size-6 shrink-0 items-center justify-center rounded-md bg-surface-4 text-sage"><Icon name=kind_icon(node.kind()) size=13 /></span>
                            <strong class="label-clamp min-w-0 text-[12.5px] leading-[1.3] font-semibold text-ink [--lines:3]"><LabelText label=node.label.clone() /></strong>
                            {move || change().map(|status| {
                                let (class, title) = if status == "broken" {
                                    ("bg-danger text-white", "A cited file is gone")
                                } else {
                                    ("bg-warning text-on-warning", "Cited code changed since this participant was checked")
                                };
                                view! { <span class=format!("absolute -top-2.5 right-2 rounded-full px-1.5 py-px text-[10px] font-bold {class}") title=title aria-label=title>"changed"</span> }
                            })}
                        </button>
                    })
                })
                .collect_view()
        };
        let top = boxes(sequence_layout::MARGIN);
        let bottom = boxes(l.lifeline_bottom);
        view! {
            <svg class="pointer-events-none absolute top-0 left-0 overflow-visible" width=l.width height=l.height aria-hidden="true">
                <defs>
                    <marker id=format!("{marker}-filled") viewBox="0 0 10 10" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto-start-reverse">
                        <path d="M0,0.5 L9.5,5 L0,9.5 z" class="fill-diagram-edge"></path>
                    </marker>
                    <marker id=format!("{marker}-async") viewBox="0 0 10 10" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" orient="auto">
                        <path d="M1,1 L9,5 L1,9" class="fill-none stroke-diagram-edge" stroke-width="1.6"></path>
                    </marker>
                </defs>
                {frame_bodies}
                {lifelines}
                {bars}
                {frames}
                {messages}
            </svg>
            {notes}
            {top}
            {bottom}
        }
    };
    if still {
        return view! {
            <div class="relative" style:width=move || format!("{}px", layout.with(|l| l.width)) style:height=move || format!("{}px", layout.with(|l| l.height))>
                {drawing}
            </div>
        }
        .into_any();
    }
    let step =
        move |by: f64| zoom.update(|z| *z = ((*z * by) * 100.0).round().clamp(40.0, 200.0) / 100.0);
    view! {
        <div
            class="relative h-full w-full overflow-auto bg-canvas"
            aria-label="Sequence diagram"
            role="region"
            on:wheel=move |ev| {
                if ev.ctrl_key() || ev.meta_key() {
                    ev.prevent_default();
                    step(if ev.delta_y() < 0.0 { 1.1 } else { 1.0 / 1.1 });
                }
            }
        >
            <div class="mx-auto" style:width=move || format!("{}px", layout.with(|l| l.width) * zoom.get()) style:height=move || format!("{}px", layout.with(|l| l.height) * zoom.get())>
                <div class="relative origin-top-left" style:transform=move || format!("scale({})", zoom.get()) style:width=move || format!("{}px", layout.with(|l| l.width)) style:height=move || format!("{}px", layout.with(|l| l.height))>
                    {drawing}
                </div>
            </div>
            <div class="sticky bottom-4 left-4 z-10 ml-4 inline-flex items-center gap-1 rounded-[10px] border border-line bg-floating p-1 shadow-lg" role="group" aria-label="Zoom">
                <button type="button" class="icon-btn size-7" aria-label="Zoom out" on:click=move |_| step(1.0 / 1.2)>"−"</button>
                <button type="button" class="min-w-12 rounded-md px-1 font-mono text-[11px] text-soft hover:bg-surface-3" aria-label="Reset zoom" on:click=move |_| zoom.set(1.0)>{move || format!("{:.0}%", zoom.get() * 100.0)}</button>
                <button type="button" class="icon-btn size-7" aria-label="Zoom in" on:click=move |_| step(1.2)>"+"</button>
            </div>
        </div>
    }
    .into_any()
}
