//! The diagram editor surface, built for direct manipulation: right-click or double-click to add,
//! drag a card's handle to connect (or drop on empty canvas to create a connected node), drag the
//! background to select several cards, scroll to pan and pinch or ⌘/Ctrl-scroll to zoom.
//! Every change is an `Edit` the parent applies to the file.
use super::geometry::Viewport;
use super::node_card::{kind_icon, DiagramNodeCard, LabelText, PointerArgs};
use crate::components::frame_value::FrameValue;
use crate::components::icons::Icon;
use crate::diagram_layout::{
    arrow, bends, bounds, group_boxes, group_members, hit, note, place_missing, plan, preview,
    rects, saved_positions, Arrow, Edit, CARD_WIDTH, SWATCHES,
};
use crate::layout::{Bounds, GroupBox, DEFAULT_GROUP_COLOR};
use crate::layout_worker;
use crate::types::{Diagram, DiagramNode, Head, Position, Stroke};
use leptos::prelude::*;
use schematlas_parser::model;
use schematlas_ui::flow_layout::{self, FlowInput, FlowLayout};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

pub const NODE_KINDS: [(&str, &str); 9] = [
    ("service", "Service"),
    ("module", "Module"),
    ("store", "Data store"),
    ("client", "Client"),
    ("external", "External system"),
    ("queue", "Queue"),
    ("worker", "Worker"),
    ("actor", "User"),
    ("note", "Note"),
];

#[derive(Clone, Debug)]
enum Drag {
    Pan {
        start: (f64, f64),
        origin: (f64, f64),
    },
    Move {
        id: String,
        start: (f64, f64),
        origins: HashMap<String, Position>,
        moved: bool,
    },
    Marquee {
        start: Position,
        screen: (f64, f64),
        base: Vec<String>,
    },
    Connect {
        from: String,
        screen: (f64, f64),
    },
}

/// How one edge is drawn.
#[derive(Clone, Debug, PartialEq)]
struct EdgeLook {
    geometry: Arrow,
    state: &'static str,
    kind: String,
    label: Option<String>,
    dashed: bool,
    thick: bool,
    hidden: bool,
    arrowhead: bool,
    both_ends: bool,
    tail_label: Option<String>,
    head_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
enum Target {
    Canvas,
    Node(String),
    Edge(usize),
}
#[derive(Clone, Debug, PartialEq)]
struct Menu {
    x: f64,
    y: f64,
    at: Position,
    target: Target,
}

const TOOL: &str = "icon-btn text-inherit hover:bg-surface-4 aria-pressed:bg-accent-soft aria-pressed:text-accent-text";
const ITEM: &str = "flex w-full items-center gap-2.5 rounded-md px-2.5 py-[7px] text-left text-xs text-text transition-colors hover:bg-surface-4 hover:text-ink";
const KEY: &str = "ml-auto font-mono text-[10px] text-faint";

fn marker(state: &str) -> String {
    let state = match state {
        "selected" => "selected",
        "active" => "active",
        _ => "idle",
    };
    format!("url(#arrow-{state})")
}

fn typing(ev: &web_sys::KeyboardEvent) -> bool {
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        .is_some_and(|t| {
            matches!(t.tag_name().as_str(), "INPUT" | "TEXTAREA" | "SELECT")
                || t.is_content_editable()
        })
}

#[component]
pub fn DiagramCanvas(
    #[prop(into)] diagram: Signal<Arc<Diagram>>,
    /// The single selected node, which the inspector shows.
    selected: RwSignal<Option<String>>,
    /// An explicit jump from search or a connection in the inspector.
    focus_node: RwSignal<Option<String>>,
    /// The node whose label is being edited in place.
    editing: RwSignal<Option<String>>,
    #[prop(into)] query: Signal<String>,
    /// Open team comments per node id.
    #[prop(into)]
    comments: Signal<HashMap<String, usize>>,
    #[prop(into)] on_edit: Callback<Edit>,
    /// Open the details panel for a node.
    #[prop(into)]
    on_details: Callback<String>,
    /// Whether the details panel covers the right side of the canvas.
    #[prop(into)]
    panel: Signal<bool>,
    /// The file's structure is written by hand (class, ER, state diagrams, DBML): nodes and
    /// connections cannot be added here, only described and arranged.
    #[prop(into)]
    locked: Signal<bool>,
    /// Forget the arrangement kept beside the file, so the diagram is laid out automatically.
    #[prop(into)]
    on_auto_layout: Callback<()>,
    /// Nodes whose cited code changed: `changed`, `broken` or `local`.
    #[prop(into)]
    changes: Signal<HashMap<String, String>>,
) -> impl IntoView {
    let container = NodeRef::<leptos::html::Div>::new();
    let size = RwSignal::new((0.0_f64, 0.0_f64));
    let viewport = RwSignal::new(Viewport::default());
    let animating = RwSignal::new(false);
    let positions = RwSignal::new(HashMap::<String, Position>::new());
    // The automatic layout last computed, with its edge routes and group boxes.
    let flow = RwSignal::new(None::<Arc<FlowLayout>>);
    let auto = Memo::new(move |_| {
        diagram.with(|d| d.kind != model::DRAWIO && saved_positions(d).is_empty())
    });
    let selection = RwSignal::new(Vec::<String>::new());
    let selected_edge = RwSignal::new(None::<usize>);
    let editing_edge = RwSignal::new(None::<usize>);
    let menu = RwSignal::new(None::<Menu>);
    let marquee = RwSignal::new(None::<Bounds>);
    let link = RwSignal::new(None::<(String, Position)>);
    let link_target = RwSignal::new(None::<String>);
    let connect_mode = RwSignal::new(false);
    let connect_from = RwSignal::new(None::<String>);
    let space = RwSignal::new(false);
    let arranging = RwSignal::new(false);
    let layout_error = RwSignal::new(String::new());
    let drag = StoredValue::new_local(None::<Drag>);
    let fitted = StoredValue::new(false);
    let layout_abort = StoredValue::new_local(None::<web_sys::AbortController>);
    on_cleanup(move || {
        layout_abort.try_update_value(|controller| {
            if let Some(controller) = controller.take() {
                controller.abort();
            }
        });
    });

    let nodes = Memo::new(move |_| {
        diagram.with(|d| {
            d.nodes
                .iter()
                .map(|n| (n.id.clone(), Arc::new(n.clone())))
                .collect::<HashMap<_, _>>()
        })
    });
    let boxes = Memo::new(move |_| diagram.with(|d| positions.with(|p| rects(d, p))));
    let matches = Memo::new(move |_| {
        let q = query.get().trim().to_lowercase();
        diagram.with(|d| {
            d.nodes
                .iter()
                .filter(|n| {
                    q.is_empty()
                        || crate::label::plain(&n.label).to_lowercase().contains(&q)
                        || n.id.to_lowercase().contains(&q)
                        || n.annotation
                            .tags
                            .iter()
                            .any(|t| t.to_lowercase().contains(&q))
                        || note(n).is_some_and(|b| b.to_lowercase().contains(&q))
                })
                .map(|n| n.id.clone())
                .collect::<HashSet<_>>()
        })
    });

    let neighborhood = Memo::new(move |_| {
        let chosen = selected.get()?;
        Some(diagram.with(|d| crate::diagram_layout::neighborhood(d, &chosen)))
    });
    // One selection model: `selection` holds every selected card, `selected` mirrors it when
    // exactly one is selected so the inspector can follow.
    let select = move |ids: Vec<String>| {
        selected.set(if ids.len() == 1 {
            ids.first().cloned()
        } else {
            None
        });
        if !ids.is_empty() {
            selected_edge.set(None);
        }
        selection.set(ids);
    };
    Effect::new(move |_| match selected.get() {
        Some(id) if !selection.with_untracked(|s| s.contains(&id)) => selection.set(vec![id]),
        None if selection.with_untracked(|s| s.len() == 1) => selection.set(vec![]),
        _ => {}
    });
    // Keep a node clear of the toolbar, the hint bar and the inspector, panning only as needed.
    let reveal = move |id: String| {
        let Some(r) = boxes.with_untracked(|b| b.get(&id).copied()) else {
            return;
        };
        let (width, height) = size.get_untracked();
        let v = viewport.get_untracked();
        let right = width - if panel.get_untracked() { 360.0 } else { 24.0 };
        let (x0, y0) = (r.x * v.zoom + v.x, r.y * v.zoom + v.y);
        let (x1, y1) = (x0 + r.w * v.zoom, y0 + r.h * v.zoom);
        let dx = if x1 > right {
            right - x1
        } else if x0 < 24.0 {
            24.0 - x0
        } else {
            0.0
        };
        let dy = if y1 > height - 64.0 {
            height - 64.0 - y1
        } else if y0 < 72.0 {
            72.0 - y0
        } else {
            0.0
        };
        if dx != 0.0 || dy != 0.0 {
            animating.set(true);
            set_timeout(
                move || {
                    animating.try_set(false);
                },
                Duration::from_millis(260),
            );
            viewport.update(|v| {
                v.x += dx;
                v.y += dy;
            });
        }
    };
    Effect::new(move |_| {
        if let Some(id) = focus_node.get() {
            // A diagram that has just opened is still being laid out: wait for the card.
            if !boxes.with(|b| b.contains_key(&id)) {
                return;
            }
            request_animation_frame(move || {
                if let Some(r) = boxes.with_untracked(|b| b.get(&id).copied()) {
                    let (width, height) = size.get_untracked();
                    let available =
                        (width - if panel.get_untracked() { 340.0 } else { 0.0 }).max(240.0);
                    let zoom = viewport.get_untracked().zoom.max(1.0);
                    viewport.set(Viewport {
                        x: available / 2.0 - (r.x + r.w / 2.0) * zoom,
                        y: height / 2.0 - (r.y + r.h / 2.0) * zoom,
                        zoom,
                    });
                }
            });
            focus_node.set(None);
        }
    });
    // Plan locally only to learn which nodes a gesture creates; the parent owns the file.
    let run = move |edit: Edit, edit_new: bool| {
        let created = diagram
            .with_untracked(|d| plan(d, edit.clone()).map(|p| p.select))
            .unwrap_or_default();
        // Until the automatic layout places it, a new card appears where it was asked for, so
        // its label can be typed at once.
        if let (true, Edit::Add { at, .. }, Some(first)) =
            (auto.get_untracked(), &edit, created.first())
        {
            let (id, at) = (first.clone(), *at);
            positions.update(|p| {
                p.insert(id, at);
            });
        }
        on_edit.run(edit);
        if let Some(first) = created.first().cloned() {
            select(created.clone());
            if edit_new {
                editing.set(Some(first.clone()));
            }
            // The parent's new diagram reaches `positions` on the next tick.
            set_timeout(move || reveal(first), Duration::from_millis(0));
        }
    };

    let rendered = Memo::new(move |_| {
        let (width, height) = size.get();
        let rect = viewport.get().visible_rect(width.max(1.0), height.max(1.0));
        let area = Bounds {
            x: rect.x - 300.0,
            y: rect.y - 300.0,
            width: rect.width + 600.0,
            height: rect.height + 600.0,
        };
        let keep: Vec<String> = selection.get().into_iter().chain(editing.get()).collect();
        boxes.with(|b| {
            let mut ids: Vec<String> = b
                .iter()
                .filter(|(id, r)| r.intersects(&area) || keep.contains(id))
                .map(|(id, _)| id.clone())
                .collect();
            ids.sort();
            ids
        })
    });
    // The automatic layout's boxes and routes hold while every card is where it put them.
    let laid_out = Memo::new(move |_| {
        let current = flow.get()?;
        let fits = positions.with(|p| {
            current.nodes.iter().all(|(id, q)| {
                p.get(id)
                    .is_some_and(|p| (p.x - q.x).abs() < 0.5 && (p.y - q.y).abs() < 0.5)
            })
        });
        fits.then_some(current)
    });
    let overlays = Memo::new(move |_| {
        diagram.with(|d| match laid_out.get() {
            Some(flow) => d
                .groups
                .iter()
                .filter_map(|g| {
                    let b = flow.groups.get(&g.id)?;
                    Some(GroupBox {
                        group_id: g.id.clone(),
                        name: g.title.clone(),
                        color: DEFAULT_GROUP_COLOR.into(),
                        count: group_members(d, &g.id).len(),
                        x: b.x,
                        y: b.y,
                        width: b.width,
                        height: b.height,
                    })
                })
                .collect(),
            None => boxes.with(|b| group_boxes(d, b)),
        })
    });

    let set_view = move |next: Viewport, animate: bool| {
        animating.set(animate);
        if animate {
            set_timeout(
                move || {
                    animating.try_set(false);
                },
                Duration::from_millis(260),
            );
        }
        viewport.set(next);
    };
    let fit = move |animate: bool| {
        let (width, height) = size.get_untracked();
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        if let Some(b) = boxes.with_untracked(|b| bounds(b.values())) {
            // Cap the zoom so a two-node diagram is not blown up, and keep it centered.
            let zoom = Viewport::fitting(b, width, height, 0.15).zoom.min(1.2);
            set_view(
                Viewport {
                    x: width / 2.0 - (b.x + b.width / 2.0) * zoom,
                    y: height / 2.0 - (b.y + b.height / 2.0) * zoom,
                    zoom,
                },
                animate,
            );
        }
    };
    let persist = move || {
        let current = positions.get_untracked();
        if !current.is_empty() {
            on_edit.run(Edit::Layout(current));
        }
    };
    // Auto layout: a diagram without positions kept beside its file is drawn the way Mermaid
    // lays it out, and laid out again whenever its structure changes. Dragging a card keeps
    // the arrangement (manual); "Tidy up" returns to automatic.
    // Lay out off the main thread; `keep` writes the result into the file (draw.io).
    let run_flow = move |input: FlowInput, keep: bool| {
        layout_error.set(String::new());
        let Ok(controller) = web_sys::AbortController::new() else {
            layout_error.set("Automatic layout is unavailable in this browser.".into());
            return;
        };
        layout_abort.update_value(|previous| {
            if let Some(previous) = previous.replace(controller.clone()) {
                previous.abort();
            }
        });
        arranging.set(true);
        leptos::task::spawn_local(async move {
            let result = layout_worker::flow(input, &controller.signal()).await;
            if controller.signal().aborted() || arranging.is_disposed() {
                return;
            }
            match result {
                Ok(result) => {
                    let first = positions.with_untracked(HashMap::is_empty);
                    positions.set(result.nodes.clone());
                    flow.set(Some(Arc::new(result)));
                    if keep {
                        persist();
                    }
                    if keep || first {
                        fit(!first);
                    }
                }
                Err(error) => layout_error.set(error),
            }
            arranging.set(false);
        });
    };
    let flow_input =
        Memo::new(move |_| auto.get().then(|| diagram.with(|d| flow_layout::input(d))));
    Effect::new(move |_| match flow_input.get() {
        Some(input) => {
            if drag.with_value(Option::is_none) {
                run_flow(input, false);
            }
        }
        None => flow.set(None),
    });
    let arrange = move || {
        if arranging.get_untracked() {
            return;
        }
        if diagram.with_untracked(|d| d.kind == model::DRAWIO) {
            // draw.io keeps positions in the file itself: write the clean layout there.
            run_flow(diagram.with_untracked(|d| flow_layout::input(d)), true);
        } else if auto.get_untracked() {
            fit(true);
        } else {
            on_auto_layout.run(());
        }
    };

    // Hand-arranged diagrams get smooth routes around the other cards too, computed once the
    // cards rest; each holds while both of its cards stay where it was computed for.
    let manual_routes =
        RwSignal::new(None::<Arc<(HashMap<String, Position>, Vec<Option<flow_layout::Route>>)>>);
    let route_abort = StoredValue::new_local(None::<web_sys::AbortController>);
    on_cleanup(move || {
        route_abort.try_update_value(|controller| {
            if let Some(controller) = controller.take() {
                controller.abort();
            }
        });
    });
    Effect::new(move |_| {
        let current = positions.get();
        let structure = diagram.with(|d| flow_layout::input(d));
        if auto.get() || current.is_empty() || drag.with_value(Option::is_some) {
            return;
        }
        let Ok(controller) = web_sys::AbortController::new() else {
            return;
        };
        route_abort.update_value(|previous| {
            if let Some(previous) = previous.replace(controller.clone()) {
                previous.abort();
            }
        });
        leptos::task::spawn_local(async move {
            let result =
                layout_worker::route(structure, current.clone(), &controller.signal()).await;
            if controller.signal().aborted() {
                return;
            }
            if let Ok(routes) = result {
                manual_routes.try_set(Some(Arc::new((current, routes))));
            }
        });
    });

    // Manual diagrams follow their saved positions; cards added since (by the agent, an
    // editor or git) are placed beside the drawing.
    Effect::new(move |_| {
        if auto.get() {
            return;
        }
        match diagram.with(|d| place_missing(d)) {
            Some(placed) => {
                if drag.with_value(Option::is_none) {
                    positions.set(placed);
                }
            }
            None => run_flow(diagram.with_untracked(|d| flow_layout::input(d)), true),
        }
    });
    Effect::new(move |_| {
        let Some(element) = container.get() else {
            return;
        };
        let element: web_sys::HtmlElement = element.into();
        let measure = move || {
            let rect = element.get_bounding_client_rect();
            size.set((rect.width(), rect.height()));
        };
        measure();
        let closure =
            Closure::<dyn FnMut(js_sys::Array)>::new(move |_entries: js_sys::Array| measure());
        if let Ok(observer) = web_sys::ResizeObserver::new(closure.as_ref().unchecked_ref()) {
            if let Some(target) = container.get_untracked() {
                observer.observe(&target);
            }
            let keep = StoredValue::new_local((closure, observer));
            on_cleanup(move || {
                keep.try_update_value(|(_, observer)| observer.disconnect());
            });
        }
    });
    Effect::new(move |_| {
        let (width, _) = size.get();
        let ready = positions.with(|p| !p.is_empty());
        if width > 0.0 && ready && !fitted.get_value() {
            fitted.set_value(true);
            fit(false);
        }
    });

    let local = move |client_x: f64, client_y: f64| -> (f64, f64) {
        container
            .get_untracked()
            .map(|el| {
                let rect = el.get_bounding_client_rect();
                (client_x - rect.left(), client_y - rect.top())
            })
            .unwrap_or((client_x, client_y))
    };
    let to_graph_point = move |client_x: f64, client_y: f64| {
        let (x, y) = local(client_x, client_y);
        viewport.get_untracked().to_graph(x, y)
    };
    let capture = move |pointer_id: i32| {
        if let Some(el) = container.get_untracked() {
            let _ = el.set_pointer_capture(pointer_id);
        }
    };
    let screen_center = move || {
        let (width, height) = size.get_untracked();
        viewport.get_untracked().to_graph(width / 2.0, height / 2.0)
    };
    let add_at = move |at: Position, kind: &str, from: Option<String>| {
        run(
            Edit::Add {
                at: Position {
                    x: at.x - CARD_WIDTH / 2.0,
                    y: at.y - 32.0,
                },
                kind: kind.to_string(),
                from,
            },
            true,
        );
    };

    let motion = FrameValue::new(move |(cx, cy): (f64, f64)| {
        let zoom = viewport.get_untracked().zoom;
        drag.update_value(|state| match state {
            Some(Drag::Pan { start, origin }) => viewport.update(|v| {
                v.x = origin.0 + cx - start.0;
                v.y = origin.1 + cy - start.1;
            }),
            Some(Drag::Move {
                start,
                origins,
                moved,
                ..
            }) => {
                if (cx - start.0).abs() + (cy - start.1).abs() > 3.0 {
                    *moved = true;
                }
                if *moved {
                    let (dx, dy) = ((cx - start.0) / zoom, (cy - start.1) / zoom);
                    positions.update(|p| {
                        for (id, origin) in origins.iter() {
                            p.insert(
                                id.clone(),
                                Position {
                                    x: origin.x + dx,
                                    y: origin.y + dy,
                                },
                            );
                        }
                    });
                }
            }
            Some(Drag::Marquee { start, .. }) => {
                let end = to_graph_point(cx, cy);
                marquee.set(Some(Bounds {
                    x: start.x.min(end.x),
                    y: start.y.min(end.y),
                    width: (end.x - start.x).abs(),
                    height: (end.y - start.y).abs(),
                }));
            }
            Some(Drag::Connect { from, .. }) => {
                let at = to_graph_point(cx, cy);
                link.set(Some((from.clone(), at)));
                link_target.set(boxes.with_untracked(|b| hit(b, at, Some(from))));
            }
            None => {}
        });
    });

    let pick = move |id: String| {
        match connect_from.get_untracked() {
            Some(from) if from != id => {
                run(
                    Edit::Connect {
                        from,
                        to: id.clone(),
                    },
                    false,
                );
                connect_from.set(None);
            }
            _ => connect_from.set(Some(id.clone())),
        }
        select(vec![id]);
    };
    let card_down = Callback::new(move |(id, x, y, pointer, shift): PointerArgs| {
        menu.set(None);
        if connect_mode.get_untracked() {
            pick(id);
            return;
        }
        if shift {
            let mut next = selection.get_untracked();
            match next.iter().position(|s| *s == id) {
                Some(i) => {
                    next.remove(i);
                }
                None => next.push(id),
            }
            select(next);
            return;
        }
        if !selection.with_untracked(|s| s.contains(&id)) {
            select(vec![id.clone()]);
        }
        motion.cancel();
        capture(pointer);
        let origins = positions.with_untracked(|p| {
            selection
                .get_untracked()
                .iter()
                .filter_map(|s| p.get(s).map(|pos| (s.clone(), *pos)))
                .collect()
        });
        drag.set_value(Some(Drag::Move {
            id,
            start: (x, y),
            origins,
            moved: false,
        }));
        animating.set(false);
    });
    let handle_down = Callback::new(move |(id, x, y, pointer, _): PointerArgs| {
        menu.set(None);
        motion.cancel();
        capture(pointer);
        drag.set_value(Some(Drag::Connect {
            from: id.clone(),
            screen: (x, y),
        }));
        link.set(Some((id, to_graph_point(x, y))));
    });
    let group_down = move |group_id: String, x: f64, y: f64, pointer: i32| {
        let members = diagram.with_untracked(|d| {
            d.groups
                .iter()
                .any(|g| g.id == group_id)
                .then(|| group_members(d, &group_id))
        });
        let Some(members) = members else { return };
        menu.set(None);
        motion.cancel();
        capture(pointer);
        select(members.clone());
        let origins = positions.with_untracked(|p| {
            members
                .iter()
                .filter_map(|id| p.get(id).map(|pos| (id.clone(), *pos)))
                .collect()
        });
        drag.set_value(Some(Drag::Move {
            id: group_id,
            start: (x, y),
            origins,
            moved: false,
        }));
    };
    let pointer_down = move |ev: leptos::ev::PointerEvent| {
        if drag.with_value(Option::is_some) || ev.button() == 2 {
            return;
        }
        menu.set(None);
        if editing.get_untracked().is_some() || editing_edge.get_untracked().is_some() {
            // Let the open editor blur and commit first.
            return;
        }
        ev.prevent_default();
        motion.cancel();
        capture(ev.pointer_id());
        let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
        if ev.button() == 1 || space.get_untracked() {
            let view = viewport.get_untracked();
            drag.set_value(Some(Drag::Pan {
                start: (x, y),
                origin: (view.x, view.y),
            }));
        } else {
            let base = if ev.shift_key() {
                selection.get_untracked()
            } else {
                vec![]
            };
            drag.set_value(Some(Drag::Marquee {
                start: to_graph_point(x, y),
                screen: (x, y),
                base,
            }));
        }
        animating.set(false);
    };
    let pointer_move = move |ev: leptos::ev::PointerEvent| {
        if drag.with_value(Option::is_some) {
            motion.push((ev.client_x() as f64, ev.client_y() as f64));
        }
    };
    let pointer_up = move |ev: leptos::ev::PointerEvent| {
        if !drag.with_value(Option::is_some) {
            return;
        }
        let (x, y) = (ev.client_x() as f64, ev.client_y() as f64);
        motion.push((x, y));
        motion.flush();
        if let Some(el) = container.get_untracked() {
            let _ = el.release_pointer_capture(ev.pointer_id());
        }
        match drag.try_update_value(|d| d.take()).flatten() {
            Some(Drag::Move { moved: true, .. }) => persist(),
            Some(Drag::Move {
                id, moved: false, ..
            }) if nodes.with_untracked(|n| n.contains_key(&id)) => select(vec![id]),
            Some(Drag::Marquee { screen, base, .. }) => {
                let area = marquee.get_untracked();
                marquee.set(None);
                let tiny = (x - screen.0).abs() + (y - screen.1).abs() < 4.0;
                match area.filter(|_| !tiny) {
                    Some(area) => {
                        let mut ids = base;
                        boxes.with_untracked(|b| {
                            for (id, r) in b {
                                if r.intersects(&area) && !ids.contains(id) {
                                    ids.push(id.clone());
                                }
                            }
                        });
                        ids.sort();
                        select(ids);
                    }
                    None => {
                        select(vec![]);
                        selected_edge.set(None);
                        connect_from.set(None);
                    }
                }
            }
            Some(Drag::Connect { from, screen }) => {
                let target = link_target.get_untracked();
                link.set(None);
                link_target.set(None);
                let far = (x - screen.0).abs() + (y - screen.1).abs() > 24.0;
                match target {
                    Some(to) => run(Edit::Connect { from, to }, false),
                    None if far => add_at(to_graph_point(x, y), "module", Some(from)),
                    None => {}
                }
            }
            _ => {}
        }
    };
    let cancel_drag = move |_ev: leptos::ev::PointerEvent| {
        motion.cancel();
        marquee.set(None);
        link.set(None);
        link_target.set(None);
        match drag.try_update_value(Option::take).flatten() {
            Some(Drag::Pan { origin, .. }) => viewport.update(|v| {
                v.x = origin.0;
                v.y = origin.1;
            }),
            Some(Drag::Move { origins, .. }) => positions.update(|p| p.extend(origins)),
            _ => {}
        }
    };
    // Trackpads scroll to pan and pinch (sent as ctrl+wheel) to zoom; ⌘/Ctrl+wheel zooms on mice.
    let wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default();
        menu.set(None);
        animating.set(false);
        if ev.ctrl_key() || ev.meta_key() {
            let (mx, my) = local(ev.client_x() as f64, ev.client_y() as f64);
            let factor = (-ev.delta_y() * 0.01).exp().clamp(0.5, 2.0);
            viewport.update(|v| *v = v.zoomed_at(factor, mx, my));
        } else {
            let (dx, dy) = if ev.shift_key() && ev.delta_x() == 0.0 {
                (ev.delta_y(), 0.0)
            } else {
                (ev.delta_x(), ev.delta_y())
            };
            viewport.update(|v| {
                v.x -= dx;
                v.y -= dy;
            });
        }
    };
    let zoom_by = move |factor: f64| {
        let (width, height) = size.get_untracked();
        set_view(
            viewport
                .get_untracked()
                .zoomed_at(factor, width / 2.0, height / 2.0),
            true,
        );
    };
    let reset_zoom = move || zoom_by(1.0 / viewport.get_untracked().zoom);
    let open_menu = move |client_x: f64, client_y: f64, target: Target| {
        let (x, y) = local(client_x, client_y);
        let (width, height) = size.get_untracked();
        menu.set(Some(Menu {
            x: x.min(width - 236.0).max(4.0),
            y: y.min(height - 380.0).max(4.0),
            at: to_graph_point(client_x, client_y),
            target,
        }));
    };
    let node_menu = Callback::new(move |(id, x, y): (String, f64, f64)| {
        if !selection.with_untracked(|s| s.contains(&id)) {
            select(vec![id.clone()]);
        }
        open_menu(x, y, Target::Node(id));
    });
    let rename = Callback::new(move |(id, label): (String, Option<String>)| {
        editing.set(None);
        if let Some(label) = label {
            if nodes.with_untracked(|n| n.get(&id).is_some_and(|n| n.label != label.trim())) {
                run(Edit::Rename { id, label }, false);
            }
        }
    });
    let nudge = move |dx: f64, dy: f64| {
        let ids = selection.get_untracked();
        positions.update(|p| {
            for id in &ids {
                if let Some(pos) = p.get_mut(id) {
                    pos.x += dx;
                    pos.y += dy;
                }
            }
        });
        persist();
    };
    let delete_selection = move || {
        let nodes = selection.get_untracked();
        let edges: Vec<usize> = selected_edge.get_untracked().into_iter().collect();
        if nodes.is_empty() && edges.is_empty() {
            return;
        }
        select(vec![]);
        selected_edge.set(None);
        run(Edit::Delete { nodes, edges }, false);
    };

    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        if typing(&ev) {
            return;
        }
        let command = ev.meta_key() || ev.ctrl_key();
        let key = ev.key();
        let handled = match key.as_str() {
            " " => {
                space.set(true);
                true
            }
            "Escape" => {
                if menu.get_untracked().is_some() {
                    menu.set(None);
                } else if connect_mode.get_untracked() {
                    connect_mode.set(false);
                    connect_from.set(None);
                } else {
                    select(vec![]);
                    selected_edge.set(None);
                }
                true
            }
            "Delete" | "Backspace" => {
                delete_selection();
                true
            }
            "a" if command => {
                select(nodes.with_untracked(|n| {
                    let mut ids: Vec<String> = n.keys().cloned().collect();
                    ids.sort();
                    ids
                }));
                true
            }
            "d" if command => {
                let ids = selection.get_untracked();
                if !ids.is_empty() {
                    run(Edit::Duplicate { ids }, false);
                }
                true
            }
            "Enter" => match selection.get_untracked().as_slice() {
                [one] => {
                    editing.set(Some(one.clone()));
                    true
                }
                _ => match selected_edge.get_untracked() {
                    Some(index) => {
                        editing_edge.set(Some(index));
                        true
                    }
                    None => false,
                },
            },
            "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" if !command => {
                let step = if ev.shift_key() { 50.0 } else { 10.0 };
                let (dx, dy) = match key.as_str() {
                    "ArrowLeft" => (-step, 0.0),
                    "ArrowRight" => (step, 0.0),
                    "ArrowUp" => (0.0, -step),
                    _ => (0.0, step),
                };
                if selection.with_untracked(Vec::is_empty) {
                    false
                } else {
                    nudge(dx, dy);
                    true
                }
            }
            "+" | "=" => {
                zoom_by(1.2);
                true
            }
            "-" | "_" if !command => {
                zoom_by(1.0 / 1.2);
                true
            }
            "!" => {
                fit(true);
                true
            }
            "0" if command => {
                reset_zoom();
                true
            }
            "c" | "C" if !command => {
                connect_mode.update(|c| *c = !*c);
                connect_from.set(None);
                true
            }
            "i" | "I" if !command => match selection.get_untracked().as_slice() {
                [one] => {
                    on_details.run(one.clone());
                    true
                }
                _ => false,
            },
            "n" | "N" if !command => {
                add_at(screen_center(), "module", None);
                true
            }
            _ => false,
        };
        if handled {
            ev.prevent_default();
        }
    });
    let keys_up = window_event_listener(leptos::ev::keyup, move |ev| {
        if ev.key() == " " {
            space.set(false);
        }
    });
    on_cleanup(move || {
        keys.remove();
        keys_up.remove();
    });

    let bend_list = Memo::new(move |_| diagram.with(|d| bends(&d.edges)));
    // Edges are keyed by position: ids written by hand may be missing or repeated until saved.
    let edge_ids = Memo::new(move |_| diagram.with(|d| (0..d.edges.len()).collect::<Vec<_>>()));
    let edge_view = move |index: usize| {
        Memo::new(move |_| {
            let edge = diagram.with(|d| d.edges.get(index).cloned())?;
            let bend = bend_list.with(|b| b.get(index).copied().unwrap_or(0.0));
            let (a, b) = boxes.with(|r| Some((*r.get(&edge.from)?, *r.get(&edge.to)?)))?;
            let routed = laid_out.with(|f| {
                let route = f.as_ref()?.routes.get(index)?.clone()?;
                let first = *route.points.first()?;
                let last = *route.points.last()?;
                let n = route.points.len();
                let near = |p: (f64, f64), q: (f64, f64)| {
                    let len = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt().max(1.0);
                    let along = 22.0_f64.min(len / 2.0);
                    let (ux, uy) = ((q.0 - p.0) / len, (q.1 - p.1) / len);
                    (p.0 + ux * along + uy * 11.0, p.1 + uy * along - ux * 11.0)
                };
                Some(Arrow {
                    path: flow_layout::route_path(&route.points, 6.0),
                    label: route.label.unwrap_or(route.points[n / 2]),
                    tail: near(first, route.points[1]),
                    head: near(last, route.points[n - 2]),
                })
            });
            let routed = routed.or_else(|| {
                if auto.get() {
                    return None;
                }
                manual_routes.with(|m| {
                    let (placed, routes) = &**m.as_ref()?;
                    let same = |id: &str| {
                        positions.with(|p| match (p.get(id), placed.get(id)) {
                            (Some(a), Some(b)) => {
                                (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5
                            }
                            _ => false,
                        })
                    };
                    if !same(&edge.from) || !same(&edge.to) {
                        return None;
                    }
                    let route = routes.get(index)?.clone()?;
                    let n = route.points.len();
                    if n < 2 {
                        return None;
                    }
                    let near = |p: (f64, f64), q: (f64, f64)| {
                        let len = ((q.0 - p.0).powi(2) + (q.1 - p.1).powi(2)).sqrt().max(1.0);
                        let along = 22.0_f64.min(len / 2.0);
                        let (ux, uy) = ((q.0 - p.0) / len, (q.1 - p.1) / len);
                        (p.0 + ux * along + uy * 11.0, p.1 + uy * along - ux * 11.0)
                    };
                    Some(Arrow {
                        path: flow_layout::route_path(&route.points, 6.0),
                        label: route.label.unwrap_or(route.points[n / 2]),
                        tail: near(route.points[0], route.points[1]),
                        head: near(route.points[n - 1], route.points[n - 2]),
                    })
                })
            });
            let geometry = routed.unwrap_or_else(|| arrow(&a, &b, bend));
            let picked = selection.get();
            let state = if selected_edge.get() == Some(index) {
                "selected"
            } else if picked.contains(&edge.from) || picked.contains(&edge.to) {
                "active"
            } else if !picked.is_empty() {
                "muted"
            } else {
                "idle"
            };
            let kind = edge.annotation.kind.clone().unwrap_or_default();
            Some(EdgeLook {
                geometry,
                state,
                dashed: edge.stroke == Stroke::Dotted
                    || matches!(kind.as_str(), "async" | "event" | "depends"),
                thick: edge.stroke == Stroke::Thick,
                hidden: edge.stroke == Stroke::Invisible,
                arrowhead: edge.head != Head::None,
                both_ends: edge.both_ends,
                kind,
                label: edge.label,
                tail_label: edge.tail_label,
                head_label: edge.head_label,
            })
        })
    };
    let preview_path = Memo::new(move |_| {
        let (from, at) = link.get()?;
        let a = boxes.with(|b| b.get(&from).copied())?;
        match link_target
            .get()
            .and_then(|t| boxes.with(|b| b.get(&t).copied()))
        {
            Some(target) => Some(arrow(&a, &target, 0.0).path),
            None => Some(preview(&a, at)),
        }
    });

    let hint = move || {
        if link.with(Option::is_some) {
            "Release on a node to connect it · release on empty canvas to create a connected node"
        } else if connect_mode.get() {
            if connect_from.with(Option::is_some) {
                "Now pick the node the arrow points to · Esc to stop connecting"
            } else {
                "Pick the node the arrow starts from · Esc to stop connecting"
            }
        } else if editing.with(Option::is_some) || editing_edge.with(Option::is_some) {
            "Enter to save · Shift+Enter for a new line · Esc to cancel"
        } else if selection.with(|s| s.len() > 1) {
            "Drag to move together · ⌘D duplicate · Delete to remove"
        } else if selection.with(|s| s.len() == 1) {
            "Enter or double-click to rename · drag a handle to connect · I for details · right-click for more"
        } else {
            "Double-click to add · drag to select · scroll to pan · ⌘/Ctrl+scroll to zoom"
        }
    };

    let menu_view = move || {
        let m = menu.get()?;
        let close = move || menu.set(None);
        let at = m.at;
        let body = match m.target.clone() {
            Target::Canvas => view! {
                <div class="eyebrow px-2.5 pt-1 pb-1.5">"ADD HERE"</div>
                <div class="grid grid-cols-2 gap-0.5">
                    {NODE_KINDS.iter().map(|(kind, label)| {
                        let kind = *kind;
                        view! {
                            <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); add_at(at, kind, None); }>
                                <Icon name=kind_icon(kind) size=14 class="text-sage" /> {*label}
                            </button>
                        }
                    }).collect_view()}
                </div>
                <div class="my-1 h-px bg-line"></div>
                <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); let mut ids: Vec<String> = nodes.with_untracked(|n| n.keys().cloned().collect()); ids.sort(); select(ids); }>
                    <Icon name="scan" size=14 /> "Select all" <span class=KEY>"⌘A"</span>
                </button>
                <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); arrange(); }>
                    <Icon name="layout-grid" size=14 /> "Tidy up layout"
                </button>
                <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); fit(true); }>
                    <Icon name="map" size=14 /> "Zoom to fit" <span class=KEY>"⇧1"</span>
                </button>
            }.into_any(),
            Target::Node(id) => {
                let ids = selection.get_untracked();
                let ids = if ids.contains(&id) { ids } else { vec![id.clone()] };
                let many = ids.len() > 1;
                let (edit_id, connect_id) = (id.clone(), id.clone());
                let (color_ids, kind_ids, dup_ids, del_ids) = (ids.clone(), ids.clone(), ids.clone(), ids);
                let clear_ids = color_ids.clone();
                view! {
                    {(!many).then(|| view! {
                        <button type="button" role="menuitem" class=ITEM on:click={let id = edit_id.clone(); move |_| { close(); on_details.run(id.clone()); }}>
                            <Icon name="eye" size=14 /> "Details…" <span class=KEY>"I"</span>
                        </button>
                        <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); editing.set(Some(edit_id.clone())); }>
                            <Icon name="pencil" size=14 /> "Rename" <span class=KEY>"↵"</span>
                        </button>
                        <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); connect_mode.set(true); connect_from.set(Some(connect_id.clone())); }>
                            <Icon name="spline" size=14 /> "Connect to…" <span class=KEY>"C"</span>
                        </button>
                    })}
                    <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); run(Edit::Duplicate { ids: dup_ids.clone() }, false); }>
                        <Icon name="plus" size=14 /> "Duplicate" <span class=KEY>"⌘D"</span>
                    </button>
                    <div class="my-1 h-px bg-line"></div>
                    <div class="eyebrow px-2.5 pt-1 pb-1.5">"COLOR"</div>
                    <div class="flex flex-wrap gap-1.5 px-2.5 pb-2">
                        {SWATCHES.iter().map(|color| {
                            let ids = color_ids.clone();
                            let color = color.to_string();
                            let swatch = color.clone();
                            view! {
                                <button type="button" class="size-5 rounded-full border border-line-strong transition-transform hover:scale-110" style:background=swatch.clone() aria-label=format!("Color {swatch}")
                                    on:click=move |_| { close(); run(Edit::Recolor { ids: ids.clone(), color: Some(color.clone()) }, false); }></button>
                            }
                        }).collect_view()}
                        <button type="button" class="flex size-5 items-center justify-center rounded-full border border-line-strong text-muted" aria-label="No color"
                            on:click=move |_| { close(); run(Edit::Recolor { ids: clear_ids.clone(), color: None }, false); }><Icon name="x" size=11 /></button>
                    </div>
                    <div class="eyebrow px-2.5 pt-1 pb-1.5">"KIND"</div>
                    <div class="flex flex-wrap gap-1 px-2.5 pb-2">
                        {NODE_KINDS.iter().map(|(kind, label)| {
                            let ids = kind_ids.clone();
                            let kind = *kind;
                            view! {
                                <button type="button" class="flex size-7 items-center justify-center rounded-md text-soft hover:bg-surface-4 hover:text-ink" title=*label aria-label=format!("Make {label}")
                                    on:click=move |_| { close(); run(Edit::Rekind { ids: ids.clone(), kind: kind.into() }, false); }>
                                    <Icon name=kind_icon(kind) size=14 />
                                </button>
                            }
                        }).collect_view()}
                    </div>
                    <div class="my-1 h-px bg-line"></div>
                    <button type="button" role="menuitem" class=format!("{ITEM} text-danger-text hover:text-danger-text") on:click=move |_| { close(); select(vec![]); run(Edit::Delete { nodes: del_ids.clone(), edges: vec![] }, false); }>
                        <Icon name="trash-2" size=14 /> {if many { "Delete selection" } else { "Delete" }} <span class=KEY>"⌫"</span>
                    </button>
                }.into_any()
            }
            Target::Edge(index) => view! {
                <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); editing_edge.set(Some(index)); }>
                    <Icon name="pencil" size=14 /> "Edit label" <span class=KEY>"↵"</span>
                </button>
                <button type="button" role="menuitem" class=ITEM on:click=move |_| { close(); run(Edit::Reverse { index }, false); }>
                    <Icon name="refresh-cw" size=14 /> "Reverse direction"
                </button>
                <div class="my-1 h-px bg-line"></div>
                <button type="button" role="menuitem" class=format!("{ITEM} text-danger-text hover:text-danger-text") on:click=move |_| { close(); selected_edge.set(None); run(Edit::Delete { nodes: vec![], edges: vec![index] }, false); }>
                    <Icon name="trash-2" size=14 /> "Delete connection" <span class=KEY>"⌫"</span>
                </button>
            }.into_any(),
        };
        Some(view! {
            <div
                role="menu"
                aria-label="Canvas actions"
                class="absolute z-30 w-[228px] animate-rise-in rounded-[10px] border border-line-strong bg-surface-2 p-1.5 shadow-[0_14px_40px_#0008]"
                style:left=format!("{}px", m.x)
                style:top=format!("{}px", m.y)
                on:pointerdown=|ev| ev.stop_propagation()
                on:dblclick=|ev| ev.stop_propagation()
                on:contextmenu=|ev| { ev.prevent_default(); ev.stop_propagation(); }
                on:wheel=|ev| ev.stop_propagation()
            >
                {body}
            </div>
        })
    };

    view! {
        <div
            node_ref=container
            class="relative min-w-0 flex-1 touch-none overflow-hidden bg-canvas outline-none select-none"
            class=("cursor-grab", move || space.get())
            class=("cursor-crosshair", move || connect_mode.get())
            aria-label="Interactive diagram"
            aria-describedby="diagram-keys"
            on:pointerdown=pointer_down
            on:pointermove=pointer_move
            on:pointerup=pointer_up
            on:pointercancel=cancel_drag
            on:lostpointercapture=cancel_drag
            on:dblclick=move |ev: leptos::ev::MouseEvent| if !locked.get_untracked() { add_at(to_graph_point(ev.client_x() as f64, ev.client_y() as f64), "module", None) }
            on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                ev.prevent_default();
                open_menu(ev.client_x() as f64, ev.client_y() as f64, Target::Canvas);
            }
            on:auxclick=|ev| { if ev.button() == 1 { ev.prevent_default(); } }
            on:wheel=wheel
        >
            <p id="diagram-keys" class="sr-only">
                "Double-click or right-click the canvas to add a node. Tab moves between nodes; Enter selects one and Enter again renames it. Arrow keys move the selection, Delete removes it, Command D duplicates, C toggles connect mode and Escape clears the selection."
            </p>
            <div
                class="absolute top-0 left-0 origin-top-left"
                class=("transition-transform", move || animating.get())
                class=("duration-250", move || animating.get())
                class=("ease-soft", move || animating.get())
                style:transform=move || viewport.get().transform()
            >
                <For each=move || overlays.get() key=|g| (g.group_id.clone(), g.name.clone(), g.color.clone()) children=move |g| {
                    let lookup = g.group_id.clone();
                    let frame = Memo::new(move |_| overlays.with(|o| o.iter().find(|x| x.group_id == lookup).map(|x| (x.x, x.y, x.width, x.height))));
                    let down = g.group_id.clone();
                    let color = g.color.clone();
                    view! {
                        <div
                            class="pointer-events-none absolute top-0 left-0 rounded-[16px] border-[1.5px] border-dashed"
                            style:transform=move || frame.get().map(|(x, y, _, _)| format!("translate({x}px, {y}px)"))
                            style:width=move || frame.get().map(|(_, _, w, _)| format!("{w}px"))
                            style:height=move || frame.get().map(|(_, _, _, h)| format!("{h}px"))
                            style:border-color=format!("{color}90")
                            style:background=format!("{color}0f")
                        >
                            <div
                                class="pointer-events-auto absolute top-2.5 left-3 flex cursor-grab items-center gap-2 rounded-md px-2 py-1 text-[11px] font-semibold tracking-wide text-ink hover:bg-surface-3 active:cursor-grabbing"
                                title="Drag to move the whole group"
                                on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                                    if ev.button() != 0 { return; }
                                    ev.stop_propagation();
                                    group_down(down.clone(), ev.client_x() as f64, ev.client_y() as f64, ev.pointer_id());
                                }
                            >
                                <span class="size-2 rounded-full" style:background=color.clone()></span>
                                {g.name.clone()}
                            </div>
                        </div>
                    }
                } />
                <svg class="pointer-events-none absolute top-0 left-0 overflow-visible" width="1" height="1" aria-hidden="true">
                    <defs>
                        {["idle", "active", "selected"].into_iter().map(|state| view! {
                            <marker id=format!("arrow-{state}") viewBox="0 0 10 10" refX="8.5" refY="5" markerUnits="userSpaceOnUse" markerWidth="13" markerHeight="13" orient="auto-start-reverse">
                                <path d="M0,0.5 L9.5,5 L0,9.5 z" class=match state { "idle" => "fill-diagram-edge", "active" => "fill-edge-active", _ => "fill-accent" }></path>
                            </marker>
                        }).collect_view()}
                    </defs>
                    <For each=move || edge_ids.get() key=|index| *index children=move |index| {
                        let edge = edge_view(index);
                        view! {
                            <Show when=move || edge.with(Option::is_some)>
                                <g>
                                    <path
                                        class="cursor-pointer [pointer-events:stroke]"
                                        stroke="transparent"
                                        stroke-width="16"
                                        fill="none"
                                        data-edge-hit=index
                                        d=move || edge.with(|e| e.as_ref().map(|e| e.geometry.path.clone()).unwrap_or_default())
                                        on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                                            if ev.button() != 0 { return; }
                                            ev.stop_propagation();
                                            menu.set(None);
                                            select(vec![]);
                                            selected_edge.set(Some(index));
                                        }
                                        on:dblclick=move |ev: leptos::ev::MouseEvent| { ev.stop_propagation(); editing_edge.set(Some(index)); }
                                        on:contextmenu=move |ev: leptos::ev::MouseEvent| {
                                            ev.prevent_default();
                                            ev.stop_propagation();
                                            select(vec![]);
                                            selected_edge.set(Some(index));
                                            open_menu(ev.client_x() as f64, ev.client_y() as f64, Target::Edge(index));
                                        }
                                    ></path>
                                    <path
                                        class="diagram-edge"
                                        class=("opacity-0", move || edge.with(|e| e.as_ref().is_some_and(|e| e.hidden)))
                                        data-state=move || edge.with(|e| e.as_ref().map(|e| e.state).unwrap_or("idle"))
                                        data-kind=move || edge.with(|e| e.as_ref().map(|e| e.kind.clone()).unwrap_or_default())
                                        data-thick=move || edge.with(|e| e.as_ref().is_some_and(|e| e.thick).then_some(""))
                                        stroke-linecap="round"
                                        stroke-dasharray=move || edge.with(|e| e.as_ref().filter(|e| e.dashed).map(|_| "8 6"))
                                        marker-end=move || edge.with(|e| e.as_ref().filter(|e| e.arrowhead).map(|e| marker(e.state)))
                                        marker-start=move || edge.with(|e| e.as_ref().filter(|e| e.arrowhead && e.both_ends).map(|e| marker(e.state)))
                                        d=move || edge.with(|e| e.as_ref().map(|e| e.geometry.path.clone()).unwrap_or_default())
                                    ></path>
                                </g>
                            </Show>
                        }
                    } />
                    {move || preview_path.get().map(|d| view! {
                        <path d=d class="fill-none stroke-accent [stroke-width:2] [stroke-dasharray:6_4]" marker-end="url(#arrow-selected)"></path>
                    })}
                </svg>
                <For each=move || rendered.get() key=|id| id.clone() children=move |id| {
                    let node_id = id.clone();
                    let node = Memo::new(move |_| nodes.with(|all| all.get(&node_id).cloned()).unwrap_or_else(|| Arc::new(DiagramNode::default())));
                    let position_id = id.clone();
                    let position = Memo::new(move |_| positions.with(|p| p.get(&position_id).map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0))));
                    let (sel_id, dim_id, edit_id, comment_id, target_id) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
                    let is_selected = Memo::new(move |_| selection.with(|s| s.contains(&sel_id)) || connect_from.with(|f| f.as_deref() == Some(sel_id.as_str())));
                    let dimmed = Memo::new(move |_| !matches.with(|m| m.contains(&dim_id)) || neighborhood.with(|n| n.as_ref().is_some_and(|n| !n.contains(&dim_id))));
                    let is_editing = Memo::new(move |_| editing.with(|e| e.as_deref() == Some(edit_id.as_str())));
                    let count = Memo::new(move |_| comments.with(|c| c.get(&comment_id).copied().unwrap_or(0)));
                    let change_id = id.clone();
                    let change = Memo::new(move |_| changes.with(|c| c.get(&change_id).cloned()));
                    let target = Memo::new(move |_| link_target.with(|t| t.as_deref() == Some(target_id.as_str())));
                    view! {
                        <DiagramNodeCard
                            node=node
                            position=position
                            selected=is_selected
                            dimmed=dimmed
                            editing=is_editing
                            comments=count
                            change=change
                            target=target
                            on_pointer_down=card_down
                            on_handle_down=handle_down
                            on_edit=Callback::new(move |id: String| { select(vec![id.clone()]); editing.set(Some(id)); })
                            on_menu=node_menu
                            on_rename=rename
                            on_select=Callback::new(move |id: String| if connect_mode.get_untracked() { pick(id) } else { select(vec![id]) })
                        />
                    }
                } />
                // Edge labels are HTML so they can be edited in place, and sit above the cards.
                <For each=move || edge_ids.get() key=|index| *index children=move |index| {
                    let edge = edge_view(index);
                    let input = NodeRef::<leptos::html::Input>::new();
                    let done = StoredValue::new(false);
                    let editing_this = Memo::new(move |_| editing_edge.get() == Some(index));
                    Effect::new(move |_| {
                        if editing_this.get() {
                            done.set_value(false);
                            if let Some(el) = input.get() {
                                let value = edge.with_untracked(|e| e.as_ref().and_then(|e| e.label.clone()).unwrap_or_default());
                                el.set_value(&value);
                                let _ = el.focus();
                                el.select();
                            }
                        }
                    });
                    let finish = move |save: bool| {
                        if done.get_value() { return; }
                        done.set_value(true);
                        editing_edge.set(None);
                        if save {
                            if let Some(el) = input.get_untracked() {
                                run(Edit::EdgeLabel { index, label: el.value() }, false);
                            }
                        }
                    };
                    view! {
                        {move || {
                            let EdgeLook { geometry, state, label, tail_label, head_label, .. } = edge.get()?;
                            let (x, y) = geometry.label;
                            let ends = [(tail_label, geometry.tail), (head_label, geometry.head)]
                                .into_iter()
                                .filter_map(|(text, (x, y))| {
                                    let text = text.filter(|t| !t.is_empty())?;
                                    Some(view! {
                                        <span
                                            class="pointer-events-none absolute top-0 left-0 z-[5] -translate-x-1/2 -translate-y-1/2 font-mono text-[10px] whitespace-nowrap text-soft"
                                            class=("opacity-40", state == "muted")
                                            style:left=format!("{x}px")
                                            style:top=format!("{y}px")
                                        >
                                            {text}
                                        </span>
                                    })
                                })
                                .collect_view();
                            if editing_this.get() {
                                return Some(view! {
                                    <input
                                        node_ref=input
                                        class="absolute top-0 left-0 z-20 w-40 -translate-x-1/2 -translate-y-1/2 rounded-md border border-accent bg-field px-2 py-1 text-center font-mono text-[11px] text-ink outline-none focus:ring-2 focus:ring-accent/25"
                                        style:left=format!("{x}px")
                                        style:top=format!("{y}px")
                                        aria-label="Connection label"
                                        placeholder="label"
                                        on:pointerdown=|ev| ev.stop_propagation()
                                        on:dblclick=|ev| ev.stop_propagation()
                                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                            ev.stop_propagation();
                                            match ev.key().as_str() {
                                                "Enter" => { ev.prevent_default(); finish(true); }
                                                "Escape" => { ev.prevent_default(); finish(false); }
                                                _ => {}
                                            }
                                        }
                                        on:blur=move |_| finish(true)
                                    />
                                }.into_any());
                            }
                            let main = label.filter(|l| !l.is_empty()).map(|text| view! {
                                <span
                                    class="absolute top-0 left-0 z-[5] max-w-[200px] -translate-x-1/2 -translate-y-1/2 cursor-pointer truncate rounded-md border bg-surface-2 px-1.5 py-0.5 font-mono text-[10px] whitespace-nowrap text-text"
                                    class=("border-accent", state == "selected")
                                    class=("border-line-strong", state != "selected")
                                    class=("opacity-40", state == "muted")
                                    style:left=format!("{x}px")
                                    style:top=format!("{y}px")
                                    title="Double-click to edit"
                                    on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                                        if ev.button() != 0 { return; }
                                        ev.stop_propagation();
                                        select(vec![]);
                                        selected_edge.set(Some(index));
                                    }
                                    on:dblclick=move |ev: leptos::ev::MouseEvent| { ev.stop_propagation(); editing_edge.set(Some(index)); }
                                >
                                    <LabelText label=text />
                                </span>
                            });
                            Some(view! { {ends} {main} }.into_any())
                        }}
                    }
                } />
                {move || {
                    let [id] = selection.get().try_into().ok()?;
                    if editing.get().is_some() || link.with(Option::is_some) || connect_mode.get() {
                        return None;
                    }
                    let r = boxes.with(|b| b.get(&id).copied())?;
                    let scale = 1.0 / viewport.get().zoom.max(0.2);
                    Some(view! {
                        <div
                            class="absolute top-0 left-0 z-20 flex origin-bottom -translate-x-1/2 items-center gap-0.5 rounded-lg border border-line-strong bg-surface-2 p-0.5 shadow-[0_6px_18px_#0006]"
                            style:left=format!("{}px", r.x + r.w / 2.0)
                            style:top=format!("{}px", r.y - 12.0)
                            style:transform=format!("translate(-50%, -100%) scale({scale:.3})")
                            on:pointerdown=|ev| ev.stop_propagation()
                            on:dblclick=|ev| ev.stop_propagation()
                        >
                            {let id = id.clone(); view! {
                                <button type="button" class="flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] text-text hover:bg-surface-4 hover:text-ink" aria-label="Details" title="Description, code references and comments (I)" on:click=move |_| on_details.run(id.clone())>
                                    <Icon name="eye" size=13 /> "Details"
                                </button>
                            }}
                            {let id = id.clone(); view! {
                                <button type="button" class="flex size-7 items-center justify-center rounded-md text-soft hover:bg-surface-4 hover:text-ink" aria-label="Rename" title="Rename (Enter)" on:click=move |_| editing.set(Some(id.clone()))>
                                    <Icon name="pencil" size=13 />
                                </button>
                            }}
                            <button type="button" class="flex size-7 items-center justify-center rounded-md text-soft hover:bg-surface-4 hover:text-ink" aria-label="More actions" title="More (right-click)"
                                on:click=move |ev: leptos::ev::MouseEvent| open_menu(ev.client_x() as f64, ev.client_y() as f64, Target::Node(id.clone()))>
                                <Icon name="ellipsis" size=14 />
                            </button>
                        </div>
                    })
                }}
                {move || marquee.get().map(|m| view! {
                    <div
                        class="pointer-events-none absolute top-0 left-0 rounded-sm border border-accent bg-accent/10"
                        style:transform=format!("translate({}px, {}px)", m.x, m.y)
                        style:width=format!("{}px", m.width)
                        style:height=format!("{}px", m.height)
                    ></div>
                })}
            </div>
            <div class="absolute top-[15px] left-[15px] z-10 flex items-center gap-1 rounded-xl border border-line bg-surface-2 p-1 text-soft shadow-[0_3px_12px_#0004]" on:pointerdown=|ev| ev.stop_propagation() on:dblclick=|ev| ev.stop_propagation() on:contextmenu=|ev| ev.stop_propagation()>
                <Show when=move || !locked.get()>
                <button type="button" class=format!("{TOOL} w-auto px-2.5") aria-label="Add node" title="Add a node (N) — or double-click / right-click the canvas" on:click=move |_| add_at(screen_center(), "module", None)>
                    <Icon name="plus" size=17 /><span class="px-1 text-xs">"Add"</span>
                </button>
                <button
                    type="button"
                    class=TOOL
                    aria-label="Connect nodes"
                    title="Connect (C): click a node, then the node it points to — or drag from a node’s handle"
                    aria-pressed=move || connect_mode.get().to_string()
                    on:click=move |_| { connect_mode.update(|c| *c = !*c); connect_from.set(None); }
                >
                    <Icon name="spline" size=17 />
                </button>
                <span class="mx-0.5 h-[15px] w-px bg-line"></span>
                </Show>
                <button type="button" class=TOOL aria-label="Auto layout" title="Tidy up the layout" disabled=move || arranging.get() on:click=move |_| arrange()>
                    <Icon name="layout-grid" size=17 />
                </button>
                <button type="button" class=TOOL aria-label="Fit diagram to screen" title="Zoom to fit (Shift+1)" on:click=move |_| fit(true)><Icon name="scan" size=17 /></button>
            </div>
            <div class="absolute bottom-[15px] left-[15px] z-10 flex items-center overflow-hidden rounded-[9px] border border-line-soft bg-surface-3 text-soft shadow-[0_2px_8px_#0003]" on:pointerdown=|ev| ev.stop_propagation() on:dblclick=|ev| ev.stop_propagation() on:contextmenu=|ev| ev.stop_propagation()>
                <button type="button" class="flex size-8 items-center justify-center hover:bg-surface-4 hover:text-ink" aria-label="Zoom out" on:click=move |_| zoom_by(1.0 / 1.2)>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14"/></svg>
                </button>
                <button type="button" class="h-8 min-w-[52px] px-1 font-mono text-[11px] hover:bg-surface-4 hover:text-ink" aria-label="Reset zoom" title="Reset zoom (⌘0)" on:click=move |_| reset_zoom()>
                    {move || format!("{:.0}%", viewport.get().zoom * 100.0)}
                </button>
                <button type="button" class="flex size-8 items-center justify-center hover:bg-surface-4 hover:text-ink" aria-label="Zoom in" on:click=move |_| zoom_by(1.2)><Icon name="plus" size=12 /></button>
            </div>
            <p class="pointer-events-none absolute bottom-[19px] left-1/2 z-10 max-w-[calc(100%-360px)] -translate-x-1/2 truncate rounded-full bg-floating px-3.5 py-1.5 text-[11px] text-muted shadow-[0_2px_8px_#0003] max-[900px]:hidden" aria-live="polite">
                {hint}
            </p>
            {menu_view}
            <Show when=move || arranging.get() || !layout_error.get().is_empty()>
                <div class="absolute top-[15px] left-1/2 z-10 -translate-x-1/2 animate-rise-in rounded-lg border border-accent-line bg-accent-soft px-4 py-2 text-xs text-accent-text" role="status">
                    {move || if arranging.get() { "Tidying up…".to_string() } else { layout_error.get() }}
                </div>
            </Show>
            <Show when=move || diagram.with(|d| d.nodes.is_empty())>
                <div class="pointer-events-none absolute top-1/2 left-1/2 z-10 w-[340px] -translate-x-1/2 -translate-y-1/2 animate-rise-in text-center text-soft">
                    <Icon name="sparkles" size=28 class="text-sage" />
                    <h3 class="mt-4 mb-2 text-[15px] font-medium text-ink">"An empty diagram"</h3>
                    <p class="text-xs leading-relaxed text-muted">"Double-click anywhere to add a node, or right-click to pick its kind. Drag from a node’s edge to connect it. You can also ask the local agent to map part of the repository."</p>
                </div>
            </Show>
        </div>
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_menu_kind_has_a_label() {
        for (kind, label) in super::NODE_KINDS {
            assert!(!kind.is_empty() && !label.is_empty());
            assert_ne!(super::kind_icon(kind), "");
        }
    }
}
