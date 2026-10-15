//! A diagram drawn inline in a doc, read-only: laid out automatically like the editor's auto
//! mode, scaled to the page's width, with a way to open it in the editor.
use super::graph::node_card::{kind_icon, LabelText};
use super::icons::Icon;
use crate::api;
use crate::diagram_layout::{card_height, color, detail_lines, CARD_WIDTH};
use crate::layout::DEFAULT_GROUP_COLOR;
use crate::layout_worker;
use crate::state::{AtlasItem, Workspace};
use crate::types::Diagram;
use leptos::prelude::*;
use leptos::task::spawn_local;
use schematlas_ui::flow_layout::{self, FlowLayout};
use std::sync::Arc;

/// Tallest a preview grows before it scales down further.
const MAX_HEIGHT: f64 = 520.0;

#[derive(Clone)]
struct Drawn {
    diagram: Arc<Diagram>,
    layout: Arc<FlowLayout>,
    width: f64,
    height: f64,
}

#[component]
pub fn DiagramPreview(
    /// The diagram's id: a file path, or `path#n` for a block of a Markdown file.
    id: String,
    #[prop(optional)] title: Option<String>,
    /// Shown as code when the diagram cannot be drawn (an unsupported Mermaid family).
    #[prop(optional)]
    source: Option<String>,
) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let drawn = RwSignal::new(None::<Drawn>);
    let failed = RwSignal::new(None::<String>);
    let frame = NodeRef::<leptos::html::Div>::new();
    let available = RwSignal::new(720.0_f64);
    let project_id = workspace.project_id.get_untracked().unwrap_or_default();
    {
        let id = id.clone();
        spawn_local(async move {
            let file = match api::get_diagram(&project_id, &id).await {
                Ok(file) => file,
                Err(e) => {
                    failed.try_set(Some(e));
                    return;
                }
            };
            let Ok(controller) = web_sys::AbortController::new() else {
                return;
            };
            match layout_worker::flow(flow_layout::input(&file.diagram), &controller.signal()).await
            {
                Ok(layout) => {
                    let width = layout
                        .nodes
                        .values()
                        .map(|p| p.x + CARD_WIDTH)
                        .chain(layout.groups.values().map(|b| b.x + b.width))
                        .fold(0.0, f64::max);
                    let height = file
                        .diagram
                        .nodes
                        .iter()
                        .filter_map(|n| layout.nodes.get(&n.id).map(|p| p.y + card_height(n)))
                        .chain(layout.groups.values().map(|b| b.y + b.height))
                        .fold(0.0, f64::max);
                    drawn.try_set(Some(Drawn {
                        diagram: Arc::new(file.diagram),
                        layout: Arc::new(layout),
                        width,
                        height,
                    }));
                }
                Err(e) => {
                    failed.try_set(Some(e));
                }
            }
        });
    }
    Effect::new(move |_| {
        if let Some(el) = frame.get() {
            let width = el.get_bounding_client_rect().width();
            if width > 0.0 {
                available.set(width - 32.0);
            }
        }
    });
    let scale = Memo::new(move |_| {
        drawn.with(|d| {
            d.as_ref()
                .map(|d| {
                    (available.get() / d.width.max(1.0))
                        .min(MAX_HEIGHT / d.height.max(1.0))
                        .min(1.0)
                })
                .unwrap_or(1.0)
        })
    });
    let open_id = id.clone();
    let heading = title.clone().filter(|t| !t.trim().is_empty());
    let label = heading.clone().unwrap_or_else(|| "Diagram".into());
    let caption = heading.unwrap_or_else(|| id.clone());
    view! {
        <figure class="not-prose my-6 overflow-hidden rounded-xl border border-line bg-canvas" aria-label=label>
            <figcaption class="flex items-center gap-2 border-b border-line-soft bg-surface px-3 py-2 text-[11px] text-muted">
                <Icon name="workflow" size=13 />
                <span class="min-w-0 flex-1 truncate">{caption}</span>
                <button type="button" class="btn min-h-6 bg-transparent px-2 py-0.5 text-[10px]" on:click=move |_| workspace.open_item(AtlasItem::Diagram(open_id.clone()))>
                    <Icon name="arrow-up-right" size=12 /> "Open"
                </button>
            </figcaption>
            <div node_ref=frame class="flex justify-center p-4">
                {move || match (drawn.get(), failed.get()) {
                    (Some(d), _) => {
                        let s = scale.get();
                        view! {
                            <div class="relative shrink-0" style:width=format!("{}px", d.width * s) style:height=format!("{}px", d.height * s)>
                                <div class="absolute top-0 left-0 origin-top-left" style:transform=format!("scale({s:.4})") style:width=format!("{}px", d.width) style:height=format!("{}px", d.height)>
                                    {drawing(&d)}
                                </div>
                            </div>
                        }.into_any()
                    }
                    (None, Some(error)) => match source.clone() {
                        Some(code) => view! { <pre class="w-full overflow-auto text-left font-mono text-[11px] text-soft">{code}</pre> }.into_any(),
                        None => view! { <p class="text-xs text-danger-text">{error}</p> }.into_any(),
                    },
                    (None, None) => view! { <p class="py-10 text-xs text-muted">"Laying out…"</p> }.into_any(),
                }}
            </div>
        </figure>
    }
}

/// Groups, edges and cards at the layout's own scale.
fn drawing(d: &Drawn) -> impl IntoView {
    let groups = d
        .diagram
        .groups
        .iter()
        .filter_map(|g| {
            let b = d.layout.groups.get(&g.id)?;
            Some(view! {
                <div
                    class="absolute rounded-[14px] border-[1.5px] border-dashed"
                    style:left=format!("{}px", b.x)
                    style:top=format!("{}px", b.y)
                    style:width=format!("{}px", b.width)
                    style:height=format!("{}px", b.height)
                    style:border-color=format!("{DEFAULT_GROUP_COLOR}90")
                    style:background=format!("{DEFAULT_GROUP_COLOR}0f")
                >
                    <span class="absolute top-2 left-3 text-[11px] font-semibold text-ink">{g.title.clone()}</span>
                </div>
            })
        })
        .collect_view();
    let edges = d
        .diagram
        .edges
        .iter()
        .zip(&d.layout.routes)
        .filter_map(|(e, route)| {
            let route = route.as_ref()?;
            let dashed = e.stroke == crate::types::Stroke::Dotted;
            let arrow = e.head != crate::types::Head::None;
            Some(view! {
                <path
                    class="diagram-edge"
                    d=flow_layout::route_path(&route.points, 6.0)
                    stroke-dasharray=dashed.then_some("8 6")
                    marker-end=arrow.then_some("url(#preview-arrow)")
                ></path>
            })
        })
        .collect_view();
    let labels = d
        .diagram
        .edges
        .iter()
        .zip(&d.layout.routes)
        .filter_map(|(e, route)| {
            let text = e.label.clone().filter(|l| !l.is_empty())?;
            let (x, y) = route.as_ref()?.label?;
            Some(view! {
                <span
                    class="absolute max-w-[200px] -translate-x-1/2 -translate-y-1/2 truncate rounded-md border border-line-strong bg-surface-2 px-1.5 py-0.5 font-mono text-[10px] whitespace-nowrap text-text"
                    style:left=format!("{x}px")
                    style:top=format!("{y}px")
                >
                    <LabelText label=text />
                </span>
            })
        })
        .collect_view();
    let cards = d
        .diagram
        .nodes
        .iter()
        .filter_map(|n| {
            let p = d.layout.nodes.get(&n.id)?;
            let accent = color(n);
            let icon_color = accent.clone();
            let details = detail_lines(n);
            Some(view! {
                <div
                    class="absolute flex flex-col rounded-[10px] border-[1.5px] border-line-strong bg-surface-2 shadow-[0_1px_2px_#0003]"
                    style:left=format!("{}px", p.x)
                    style:top=format!("{}px", p.y)
                    style:width=format!("{CARD_WIDTH}px")
                    style:height=format!("{}px", card_height(n))
                    style:border-color=accent
                >
                    <div class="flex shrink-0 items-center gap-2.5 px-3.5" style:height=format!("{}px", crate::diagram_layout::label_height(n))>
                        <span class="flex size-7 shrink-0 items-center justify-center rounded-md bg-surface-4 text-sage" style:color=icon_color>
                            <Icon name=kind_icon(n.kind()) size=15 />
                        </span>
                        <strong class="label-clamp min-w-0 flex-1 text-[13px] leading-[1.3] font-semibold text-ink [overflow-wrap:anywhere]" style:--lines=crate::label::shown_lines(&n.label).max(2).to_string()><LabelText label=n.label.clone() /></strong>
                    </div>
                    {crate::diagram_layout::note(n).map(|text| view! {
                        <p class="-mt-2 line-clamp-2 px-3.5 text-[11px] leading-[1.45] text-muted">{text.replace(['*', '`', '#'], "")}</p>
                    })}
                    {(!details.is_empty()).then(|| view! {
                        <ul class="mx-2 mb-2 flex flex-col overflow-hidden rounded-md border border-line-soft bg-surface-3 py-[3px] font-mono text-[10.5px] text-soft">
                            {details.into_iter().map(|line| view! { <li class="h-[18px] truncate px-2 leading-[18px]">{line}</li> }).collect_view()}
                        </ul>
                    })}
                </div>
            })
        })
        .collect_view();
    view! {
        {groups}
        <svg class="pointer-events-none absolute top-0 left-0 overflow-visible" width="1" height="1" aria-hidden="true">
            <defs>
                <marker id="preview-arrow" viewBox="0 0 10 10" refX="8.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="13" markerHeight="13" orient="auto-start-reverse">
                    <path d="M0,0.5 L9.5,5 L0,9.5 z" class="fill-diagram-edge"></path>
                </marker>
            </defs>
            {edges}
        </svg>
        {cards}
        {labels}
    }
}
