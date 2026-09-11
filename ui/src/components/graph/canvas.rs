//! The schema map: a pan/zoom viewport with table cards, relationship edges and group overlays.
//! Only cards inside the viewport are mounted; positions are the model, the DOM follows.
use super::card::{CardState, EntityCard, Ports};
use super::geometry::{bezier_path, intersects, node_right, port_y, Viewport, MAX_ZOOM, MIN_ZOOM, OVERVIEW_ZOOM};
use crate::components::icons::Icon;
use crate::layout::{
    entity_height, filter_graph, graph_bounds, grid_positions, group_bounds, matching_ids, translate_group, GroupBox,
    PositionResolver, NODE_WIDTH,
};
use crate::relationships::{neighborhood, relationships, Relationship};
use crate::smart_layout::smart_layout;
use crate::types::{Entity, Position, Source};
use leptos::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Debug)]
enum Drag {
    Pan { start: (f64, f64), origin: (f64, f64), moved: bool },
    Node { id: String, start: (f64, f64), snapshot: HashMap<String, Position>, moved: bool },
    Group { members: Vec<String>, start: (f64, f64), snapshot: HashMap<String, Position> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EdgeState {
    Idle,
    Active,
    Muted,
}

#[derive(Clone, Debug, PartialEq)]
struct EdgeView {
    path: String,
    state: EdgeState,
    labels: Vec<(f64, f64, String)>,
}

const TOOL_BUTTON: &str = "icon-btn text-inherit hover:bg-surface-4 aria-pressed:bg-surface-4";
const CONTROL: &str = "flex size-[29px] items-center justify-center border-b border-line-soft bg-surface text-soft transition-colors last:border-b-0 hover:bg-surface-3 hover:text-ink";

#[component]
pub fn GraphCanvas(
    #[prop(into)] source: Signal<Arc<Source>>,
    #[prop(into)] query: Signal<String>,
    #[prop(into)] namespaces: Signal<Vec<String>>,
    #[prop(into)] related: Signal<bool>,
    #[prop(into)] focus_node_ids: Signal<Vec<String>>,
    #[prop(into)] on_select: Callback<Option<Entity>>,
    #[prop(into)] on_save: Callback<HashMap<String, Position>>,
) -> impl IntoView {
    let container = NodeRef::<leptos::html::Div>::new();
    let size = RwSignal::new((0.0_f64, 0.0_f64));
    let viewport = RwSignal::new(Viewport::default());
    let animating = RwSignal::new(false);
    let positions = RwSignal::new(HashMap::<String, Position>::new());
    let selected = RwSignal::new(None::<String>);
    let minimap = RwSignal::new(false);
    let arranging = RwSignal::new(false);
    let layout_error = RwSignal::new(String::new());
    let drag = StoredValue::new_local(None::<Drag>);
    let resolver = StoredValue::new_local(PositionResolver::new(grid_positions));
    let fitted = StoredValue::new(false);

    // Derived graph views.
    let entities = Memo::new(move |_| {
        source.with(|s| s.graph.entities.iter().map(|e| (e.id.clone(), Arc::new(e.clone()))).collect::<HashMap<_, _>>())
    });
    let ports = Memo::new(move |_| {
        source.with(|s| {
            let mut map: HashMap<String, Ports> = HashMap::new();
            for r in &s.graph.relations {
                if let Some(field) = &r.source_field {
                    map.entry(r.source.clone()).or_default().outgoing.insert(field.clone());
                }
                if let Some(field) = &r.target_field {
                    map.entry(r.target.clone()).or_default().incoming.insert(field.clone());
                }
            }
            map.into_iter().map(|(k, v)| (k, Arc::new(v))).collect::<HashMap<_, _>>()
        })
    });
    let filtered = Memo::new(move |_| source.with(|s| filter_graph(&s.graph, &namespaces.get(), related.get())));
    let visible = Memo::new(move |_| filtered.with(|g| matching_ids(g, &query.get())));
    let neighbors = Memo::new(move |_| source.with(|s| neighborhood(&s.graph, selected.get().as_deref())));
    let relations: Memo<Arc<Vec<Relationship>>> = Memo::new(move |_| source.with(|s| Arc::new(relationships(&s.graph, s.is_database()))));
    let zoom = Signal::derive(move || viewport.get().zoom);
    let overview = Memo::new(move |_| viewport.get().zoom < OVERVIEW_ZOOM);

    // Cards inside the viewport (with a margin so edges of the screen stay populated).
    let rendered = Memo::new(move |_| {
        let (width, height) = size.get();
        let rect = viewport.get().visible_rect(width.max(1.0), height.max(1.0));
        let margin = 300.0;
        let area = crate::layout::Bounds { x: rect.x - margin, y: rect.y - margin, width: rect.width + 2.0 * margin, height: rect.height + 2.0 * margin };
        let visible = visible.get();
        positions.with(|p| {
            entities.with(|all| {
                let mut ids: Vec<String> = all
                    .iter()
                    .filter(|(id, e)| {
                        visible.contains(*id)
                            && p.get(*id).is_some_and(|pos| intersects(&area, pos.x, pos.y, NODE_WIDTH, entity_height(e)))
                    })
                    .map(|(id, _)| id.clone())
                    .collect();
                ids.sort();
                ids
            })
        })
    });
    let rendered_set = Memo::new(move |_| rendered.with(|r| r.iter().cloned().collect::<HashSet<String>>()));

    let overlays = Memo::new(move |_| {
        let visible = visible.get();
        source.with(|s| {
            let graph = crate::types::Graph {
                entities: s.graph.entities.iter().filter(|e| visible.contains(&e.id)).cloned().collect(),
                relations: vec![],
                warnings: vec![],
            };
            positions.with(|p| group_bounds(s.groups(), &graph, p))
        })
    });
    let overlay_ids = Memo::new(move |_| overlays.with(|o| o.iter().map(|g| g.group_id.clone()).collect::<Vec<_>>()));

    // Reset the model whenever the source changes; keep the selection.
    Effect::new(move |_| {
        let s = source.get();
        let resolved = resolver.try_update_value(|r| r.resolve(&s.graph, &s.positions)).unwrap_or_default();
        positions.set(resolved);
        drag.set_value(None);
    });

    let set_view = move |next: Viewport, animate: bool| {
        if animate {
            animating.set(true);
            set_timeout(move || animating.set(false), Duration::from_millis(260));
        } else {
            animating.set(false);
        }
        viewport.set(next);
    };
    let fit = move |all: bool, animate: bool| {
        let (width, height) = size.get_untracked();
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let visible = visible.get_untracked();
        let chosen: HashSet<String> = if all {
            HashSet::new()
        } else {
            focus_node_ids.get_untracked().into_iter().filter(|id| visible.contains(id)).collect()
        };
        let ids = if chosen.is_empty() { &visible } else { &chosen };
        let bounds = source.with_untracked(|s| positions.with_untracked(|p| graph_bounds(&s.graph, p, Some(ids))));
        if let Some(bounds) = bounds {
            set_view(Viewport::fitting(bounds, width, height, 0.15), animate);
        }
    };
    let persist = move || {
        let current = positions.get_untracked();
        if !current.is_empty() {
            on_save.run(current);
        }
    };
    let arrange = move || {
        if arranging.get_untracked() {
            return;
        }
        arranging.set(true);
        layout_error.set(String::new());
        // Let the status pill paint before the synchronous layout runs.
        set_timeout(
            move || {
                let (graph, groups) = source.with_untracked(|s| (s.graph.clone(), s.groups().to_vec()));
                let result = smart_layout(&graph, &groups);
                if result.len() != graph.entities.len() {
                    layout_error.set("Could not arrange the graph. Try again.".into());
                } else {
                    positions.set(result);
                    persist();
                    fit(true, true);
                }
                arranging.set(false);
            },
            Duration::from_millis(30),
        );
    };

    // Measure the canvas; the first fit waits for a real size.
    Effect::new(move |_| {
        let Some(element) = container.get() else { return };
        let element: web_sys::HtmlElement = element.into();
        let measure = move || {
            let rect = element.get_bounding_client_rect();
            size.set((rect.width(), rect.height()));
        };
        measure();
        let closure = Closure::<dyn FnMut(js_sys::Array)>::new(move |_entries: js_sys::Array| measure());
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
            fit(false, false);
            let missing = source.with_untracked(|s| {
                s.graph.entities.iter().any(|e| !s.positions.get(&e.id).is_some_and(|p| p.x.is_finite() && p.y.is_finite()))
            });
            if missing {
                arrange();
            }
        }
    });
    // Filter changes glide to the new bounds once the first fit happened.
    Effect::new(move |_| {
        query.track();
        namespaces.track();
        related.track();
        focus_node_ids.track();
        if fitted.get_value() {
            fit(false, true);
        }
    });

    let client_offset = move |client_x: f64, client_y: f64| -> (f64, f64) {
        container
            .get_untracked()
            .map(|el| {
                let rect = el.get_bounding_client_rect();
                (client_x - rect.left(), client_y - rect.top())
            })
            .unwrap_or((client_x, client_y))
    };
    let capture = move |pointer_id: i32| {
        if let Some(el) = container.get_untracked() {
            let _ = el.set_pointer_capture(pointer_id);
        }
    };
    let card_down = Callback::new(move |(id, client_x, client_y, pointer_id): (String, f64, f64, i32)| {
        capture(pointer_id);
        drag.set_value(Some(Drag::Node { id, start: (client_x, client_y), snapshot: positions.get_untracked(), moved: false }));
        animating.set(false);
    });
    let group_down = Callback::new(move |(group_id, client_x, client_y, pointer_id): (String, f64, f64, i32)| {
        let members = source.with_untracked(|s| s.groups().iter().find(|g| g.id == group_id).map(|g| g.node_ids.clone()));
        let Some(members) = members else { return };
        capture(pointer_id);
        drag.set_value(Some(Drag::Group { members, start: (client_x, client_y), snapshot: positions.get_untracked() }));
        animating.set(false);
    });
    let nudge = Callback::new(move |(group_id, dx, dy): (String, f64, f64)| {
        let members = source.with_untracked(|s| s.groups().iter().find(|g| g.id == group_id).map(|g| g.node_ids.clone()));
        let Some(members) = members else { return };
        positions.update(|p| *p = translate_group(p, &members, Position { x: dx, y: dy }));
        persist();
    });
    let pointer_down = move |ev: leptos::ev::PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        capture(ev.pointer_id());
        let view = viewport.get_untracked();
        drag.set_value(Some(Drag::Pan { start: (ev.client_x() as f64, ev.client_y() as f64), origin: (view.x, view.y), moved: false }));
        animating.set(false);
    };
    let pointer_move = move |ev: leptos::ev::PointerEvent| {
        let (cx, cy) = (ev.client_x() as f64, ev.client_y() as f64);
        let zoom = viewport.get_untracked().zoom;
        drag.update_value(|state| match state {
            Some(Drag::Pan { start, origin, moved }) => {
                let (dx, dy) = (cx - start.0, cy - start.1);
                if dx.abs() + dy.abs() > 2.0 {
                    *moved = true;
                }
                viewport.update(|v| {
                    v.x = origin.0 + dx;
                    v.y = origin.1 + dy;
                });
            }
            Some(Drag::Node { id, start, snapshot, moved }) => {
                let (dx, dy) = ((cx - start.0) / zoom, (cy - start.1) / zoom);
                if (cx - start.0).abs() + (cy - start.1).abs() > 3.0 {
                    *moved = true;
                }
                if *moved {
                    if let Some(origin) = snapshot.get(id) {
                        let next = Position { x: origin.x + dx, y: origin.y + dy };
                        positions.update(|p| {
                            p.insert(id.clone(), next);
                        });
                    }
                }
            }
            Some(Drag::Group { members, start, snapshot }) => {
                let delta = Position { x: (cx - start.0) / zoom, y: (cy - start.1) / zoom };
                positions.set(translate_group(snapshot, members, delta));
            }
            None => {}
        });
    };
    let pointer_up = move |_ev: leptos::ev::PointerEvent| {
        let state = drag.try_update_value(|d| d.take()).flatten();
        match state {
            Some(Drag::Pan { moved: false, .. }) => {
                selected.set(None);
                on_select.run(None);
            }
            Some(Drag::Node { id, moved: false, .. }) => selected.set(Some(id)),
            Some(Drag::Node { moved: true, .. }) | Some(Drag::Group { .. }) => persist(),
            _ => {}
        }
    };
    let wheel = move |ev: leptos::ev::WheelEvent| {
        ev.prevent_default();
        let (mx, my) = client_offset(ev.client_x() as f64, ev.client_y() as f64);
        let factor = (-ev.delta_y() * 0.0015).exp().clamp(0.5, 2.0);
        animating.set(false);
        viewport.update(|v| *v = v.zoomed_at(factor, mx, my));
    };
    let zoom_by = move |factor: f64| {
        let (width, height) = size.get_untracked();
        let next = viewport.get_untracked().zoomed_at(factor, width / 2.0, height / 2.0);
        set_view(next, true);
    };
    let inspect = Callback::new(move |entity: Arc<Entity>| on_select.run(Some((*entity).clone())));

    let edge_ids = Memo::new(move |_| relations.with(|r| r.iter().map(|rel| rel.id.clone()).collect::<Vec<_>>()));
    let edge_view = move |edge_id: String| {
        Memo::new(move |_| {
            let rel = relations.with(|list| list.iter().find(|r| r.id == edge_id).cloned())?;
            let visible = visible.get();
            if !visible.contains(&rel.source) || !visible.contains(&rel.target) {
                return None;
            }
            let shown = rendered_set.with(|r| r.contains(&rel.source) || r.contains(&rel.target));
            if !shown {
                return None;
            }
            let (source_entity, target_entity) = entities.with(|all| (all.get(&rel.source).cloned(), all.get(&rel.target).cloned()));
            let (source_entity, target_entity) = (source_entity?, target_entity?);
            let (sp, tp) = positions.with(|p| (p.get(&rel.source).copied(), p.get(&rel.target).copied()));
            let (sp, tp) = (sp?, tp?);
            let sx = node_right(sp);
            let sy = sp.y + port_y(&source_entity, rel.source_field.as_deref());
            let tx = tp.x;
            let ty = tp.y + port_y(&target_entity, rel.target_field.as_deref());
            let current = selected.get();
            let state = match current {
                Some(id) if id == rel.source || id == rel.target => EdgeState::Active,
                Some(_) => EdgeState::Muted,
                None => EdgeState::Idle,
            };
            let mut labels = Vec::new();
            if state == EdgeState::Active {
                if !rel.source_cardinality.is_empty() {
                    labels.push((sx + 34.0, sy - 12.0, rel.source_cardinality.clone()));
                }
                if !rel.target_cardinality.is_empty() {
                    labels.push((tx - 34.0, ty - 12.0, rel.target_cardinality.clone()));
                }
            }
            Some(EdgeView { path: bezier_path(sx, sy, tx, ty), state, labels })
        })
    };
    let description = move |edge_id: String| relations.with_untracked(|list| list.iter().find(|r| r.id == edge_id).map(|r| r.description.clone()).unwrap_or_default());

    let minimap_view = Memo::new(move |_| {
        let visible = visible.get();
        let bounds = source.with(|s| positions.with(|p| graph_bounds(&s.graph, p, Some(&visible))))?;
        let pad = 40.0;
        let boxes: Vec<(f64, f64, f64, f64)> = source.with(|s| {
            positions.with(|p| {
                s.graph
                    .entities
                    .iter()
                    .filter(|e| visible.contains(&e.id))
                    .filter_map(|e| p.get(&e.id).map(|pos| (pos.x, pos.y, NODE_WIDTH, entity_height(e))))
                    .collect()
            })
        });
        let (width, height) = size.get();
        let rect = viewport.get().visible_rect(width.max(1.0), height.max(1.0));
        let view_box = format!("{} {} {} {}", bounds.x - pad, bounds.y - pad, bounds.width + 2.0 * pad, bounds.height + 2.0 * pad);
        Some((view_box, boxes, rect))
    });

    view! {
        <div
            node_ref=container
            class="relative min-w-0 flex-1 touch-none overflow-hidden bg-canvas select-none"
            style:background-image="radial-gradient(#272d33 1px, transparent 1px)"
            style:background-size=move || { let z = viewport.get().zoom * 22.0; format!("{z}px {z}px") }
            style:background-position=move || { let v = viewport.get(); format!("{}px {}px", v.x, v.y) }
            aria-label="Interactive schema graph"
            on:pointerdown=pointer_down
            on:pointermove=pointer_move
            on:pointerup=pointer_up
            on:pointercancel=pointer_up
            on:wheel=wheel
        >
            <div
                class="absolute top-0 left-0 origin-top-left"
                class=("transition-transform", move || animating.get())
                class=("duration-250", move || animating.get())
                class=("ease-soft", move || animating.get())
                style:transform=move || viewport.get().transform()
            >
                // Group overlays sit behind the cards; only their title bar takes pointer events.
                <For each=move || overlay_ids.get() key=|id| id.clone() children=move |group_id| {
                    let lookup_id = group_id.clone();
                    let overlay = Memo::new(move |_| overlays.with(|o| o.iter().find(|g| g.group_id == lookup_id).cloned()));
                    let down_id = group_id.clone();
                    let nudge_id = group_id.clone();
                    view! {
                        {move || overlay.get().map(|g: GroupBox| {
                            let down_id = down_id.clone();
                            let nudge_id = nudge_id.clone();
                            view! {
                                <div
                                    class="pointer-events-none absolute top-0 left-0 rounded-[14px] border border-dashed"
                                    style:transform=format!("translate({}px, {}px)", g.x, g.y)
                                    style:width=format!("{}px", g.width)
                                    style:height=format!("{}px", g.height)
                                    style:border-color=format!("{}80", g.color)
                                    style:background=format!("{}08", g.color)
                                    style=("--group-color", g.color.clone())
                                >
                                    <button
                                        type="button"
                                        class="pointer-events-auto flex w-full cursor-grab touch-none items-center gap-[9px] rounded-t-[13px] bg-[#11161b] px-4 py-[13px] text-left font-mono text-xs font-semibold whitespace-nowrap text-[#d5dce2] transition-colors hover:bg-[#192027] focus-visible:outline-(--group-color) focus-visible:-outline-offset-3 active:cursor-grabbing"
                                        aria-label=format!("Move group {}", g.name)
                                        title="Drag to move all members. Arrow keys move 10px; Shift moves 50px."
                                        on:pointerdown=move |ev: leptos::ev::PointerEvent| {
                                            if ev.button() != 0 { return; }
                                            ev.stop_propagation();
                                            group_down.run((down_id.clone(), ev.client_x() as f64, ev.client_y() as f64, ev.pointer_id()));
                                        }
                                        on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                            let step = if ev.shift_key() { 50.0 } else { 10.0 };
                                            let delta = match ev.key().as_str() {
                                                "ArrowLeft" => (-step, 0.0),
                                                "ArrowRight" => (step, 0.0),
                                                "ArrowUp" => (0.0, -step),
                                                "ArrowDown" => (0.0, step),
                                                _ => return,
                                            };
                                            ev.prevent_default();
                                            ev.stop_propagation();
                                            nudge.run((nudge_id.clone(), delta.0, delta.1));
                                        }
                                    >
                                        <Icon name="grip-vertical" size=14 class="text-[#8c969e]" />
                                        <span class="size-1.5 rounded-sm" style:background=g.color.clone()></span>
                                        {g.name.clone()}
                                        <small class="ml-2 opacity-55">{g.count}</small>
                                    </button>
                                </div>
                            }
                        })}
                    }
                } />
                <svg class="pointer-events-none absolute top-0 left-0 overflow-visible" width="1" height="1" aria-hidden="true">
                    <For each=move || edge_ids.get() key=|id| id.clone() children=move |edge_id| {
                        let view_memo = edge_view(edge_id.clone());
                        let title = description(edge_id.clone());
                        view! {
                            {move || view_memo.get().map(|edge| view! {
                                <g>
                                    <title>{title.clone()}</title>
                                    <path
                                        class="edge-path"
                                        data-state=match edge.state { EdgeState::Active => "active", EdgeState::Muted => "muted", EdgeState::Idle => "idle" }
                                        d=edge.path.clone()
                                    ></path>
                                    {edge.labels.iter().map(|(x, y, text)| view! {
                                        <g class="animate-fade-in" transform=format!("translate({x},{y})")>
                                            <rect class="fill-accent-soft stroke-[#668774] [stroke-width:1]" x="-24" y="-12" width="48" height="23" rx="5"></rect>
                                            <text class="fill-accent-text font-mono text-xs font-semibold" text-anchor="middle" dominant-baseline="central">{text.clone()}</text>
                                        </g>
                                    }).collect_view()}
                                </g>
                            })}
                        }
                    } />
                </svg>
                <For each=move || rendered.get() key=|id| id.clone() children=move |id| {
                    let entity_id = id.clone();
                    let entity = Memo::new(move |_| entities.with(|all| all.get(&entity_id).cloned()).unwrap_or_default());
                    let position_id = id.clone();
                    let position = Memo::new(move |_| positions.with(|p| p.get(&position_id).map(|p| (p.x, p.y)).unwrap_or((0.0, 0.0))));
                    let state_id = id.clone();
                    let state = Memo::new(move |_| match selected.get() {
                        None => CardState::Idle,
                        Some(ref s) if *s == state_id => CardState::Selected,
                        Some(_) if neighbors.with(|n| n.contains(&state_id)) => CardState::Related,
                        Some(_) => CardState::Dimmed,
                    });
                    let ports_id = id.clone();
                    let card_ports = Memo::new(move |_| ports.with(|all| all.get(&ports_id).cloned()).unwrap_or_default());
                    view! {
                        <EntityCard entity=entity position=position state=state ports=card_ports zoom=zoom on_pointer_down=card_down on_inspect=inspect />
                    }
                } />
            </div>
            <div class="absolute bottom-[15px] left-[15px] z-10 flex flex-col overflow-hidden rounded-[7px] border border-line-soft shadow-[0_2px_5px_#0002]">
                <button type="button" class=CONTROL aria-label="Zoom in" on:pointerdown=|ev| ev.stop_propagation() on:click=move |_| zoom_by(1.2)><Icon name="plus" size=12 /></button>
                <button type="button" class=CONTROL aria-label="Zoom out" on:pointerdown=|ev| ev.stop_propagation() on:click=move |_| zoom_by(1.0 / 1.2)>
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 12h14"/></svg>
                </button>
            </div>
            <div class="absolute top-[15px] right-[15px] z-10 flex items-center gap-1 rounded-[7px] border border-line-soft bg-surface-3 p-[3px] text-[#a6b0b9] shadow-[0_3px_12px_#0004]" on:pointerdown=|ev| ev.stop_propagation()>
                <button type="button" class=TOOL_BUTTON aria-label="Auto layout" title="Arrange tables by domain and relationships" disabled=move || arranging.get() on:click=move |_| arrange()>
                    <Icon name="layout-grid" size=17 />
                </button>
                <button type="button" class=TOOL_BUTTON aria-label="Fit graph to screen" on:click=move |_| fit(true, true)><Icon name="scan" size=17 /></button>
                <span class="mx-0.5 h-[15px] w-px bg-line"></span>
                <button type="button" class=TOOL_BUTTON aria-label="Toggle minimap" aria-pressed=move || minimap.get().to_string() on:click=move |_| minimap.update(|m| *m = !*m)>
                    <Icon name="map" size=17 />
                </button>
            </div>
            <Show when=move || minimap.get()>
                {move || minimap_view.get().map(|(view_box, boxes, rect)| view! {
                    <svg
                        class="absolute right-[15px] bottom-1 z-10 h-[95px] w-[145px] animate-fade-in rounded-md border border-line-soft bg-surface"
                        viewBox=view_box
                        preserveAspectRatio="xMidYMid meet"
                        aria-label="Minimap"
                        on:pointerdown=|ev| ev.stop_propagation()
                    >
                        {boxes.iter().map(|(x, y, w, h)| view! {
                            <rect x=*x y=*y width=*w height=*h rx="8" fill=move || if source.with(|s| s.is_database()) { "#475460" } else { "#526d5e" }></rect>
                        }).collect_view()}
                        <rect x=rect.x y=rect.y width=rect.width height=rect.height fill="rgba(6,8,10,0.35)" stroke="#55e58b" stroke-width=move || 2.0 / viewport.get().zoom.max(0.05) vector-effect="non-scaling-stroke"></rect>
                    </svg>
                })}
            </Show>
            <Show when=move || arranging.get() || !layout_error.get().is_empty()>
                <div class="absolute bottom-5 left-1/2 z-10 -translate-x-1/2 animate-rise-in rounded-lg border border-accent-line bg-accent-soft px-4 py-2.5 text-xs text-accent-text" role="status">
                    {move || if arranging.get() { "Arranging domains…".to_string() } else { layout_error.get() }}
                </div>
            </Show>
            <Show when=move || visible.with(|v| v.is_empty())>
                <div class="pointer-events-none absolute top-1/2 left-1/2 z-10 min-w-[270px] -translate-x-1/2 -translate-y-1/2 animate-rise-in rounded-xl bg-[#17191cee] p-[25px] text-center text-[#bdbfc2]">
                    <Icon name="search-x" size=30 />
                    <h3 class="mt-[15px] mb-2 font-medium">{move || if query.get().is_empty() { "No visible schema objects" } else { "No matching nodes" }}</h3>
                    <p class="text-[11px]">
                        {move || if query.get().is_empty() { "This source contains no objects visible to this connection." } else { "Try a table, endpoint, model, or column name." }}
                    </p>
                </div>
            </Show>
        </div>
    }
}

#[allow(dead_code)]
fn _zoom_limits() -> (f64, f64) {
    (MIN_ZOOM, MAX_ZOOM)
}
