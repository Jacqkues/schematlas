//! Diagram geometry and editing, free of any UI. Cards are compact boxes; arrows run border to
//! border along the line between card centers, bowing apart when two nodes share several edges.
//! Automatic layout reuses the schema worker by converting a diagram to its layout graph.
use crate::layout::{Bounds, GroupBox, DEFAULT_GROUP_COLOR};
use crate::types::{
    CanvasGroup, Diagram, DiagramEdge, DiagramNode, Entity, Field, Frame, Graph, Position, Relation,
};
use std::collections::{HashMap, HashSet};

pub const CARD_WIDTH: f64 = 220.0;
pub const LABEL_HEIGHT: f64 = 64.0;
pub const BODY_HEIGHT: f64 = 44.0;
/// Gap between an arrow's end and the card border, so arrowheads stay visible.
const ARROW_GAP: f64 = 6.0;

fn has_body(node: &DiagramNode) -> bool {
    node.body_md
        .as_deref()
        .is_some_and(|b| !b.trim().is_empty())
}
/// The selected component and its directly connected neighbors, in either direction.
pub fn neighborhood(diagram: &Diagram, selected: &str) -> HashSet<String> {
    let mut ids = HashSet::from([selected.to_string()]);
    for edge in &diagram.edges {
        if edge.source == selected {
            ids.insert(edge.target.clone());
        }
        if edge.target == selected {
            ids.insert(edge.source.clone());
        }
    }
    ids
}

pub fn card_height(node: &DiagramNode) -> f64 {
    if has_body(node) {
        LABEL_HEIGHT + BODY_HEIGHT
    } else {
        LABEL_HEIGHT
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
impl Rect {
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn contains(&self, p: Position) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }
    pub fn intersects(&self, b: &Bounds) -> bool {
        self.x < b.x + b.width
            && self.x + self.w > b.x
            && self.y < b.y + b.height
            && self.y + self.h > b.y
    }
    /// Where the ray from the center towards (tx, ty) leaves the box, pushed out by `gap`.
    fn exit(&self, tx: f64, ty: f64, gap: f64) -> (f64, f64) {
        let (cx, cy) = self.center();
        let (dx, dy) = (tx - cx, ty - cy);
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            return (cx, cy);
        }
        let sx = if dx.abs() > 1e-9 {
            (self.w / 2.0 + gap) / dx.abs()
        } else {
            f64::INFINITY
        };
        let sy = if dy.abs() > 1e-9 {
            (self.h / 2.0 + gap) / dy.abs()
        } else {
            f64::INFINITY
        };
        let s = sx.min(sy).min(1.0);
        (cx + dx * s, cy + dy * s)
    }
}

/// Card boxes for every placed node.
pub fn rects(diagram: &Diagram, positions: &HashMap<String, Position>) -> HashMap<String, Rect> {
    diagram
        .nodes
        .iter()
        .filter_map(|n| {
            let p = positions.get(&n.id)?;
            Some((
                n.id.clone(),
                Rect {
                    x: p.x,
                    y: p.y,
                    w: CARD_WIDTH,
                    h: card_height(n),
                },
            ))
        })
        .collect()
}
pub fn bounds<'a>(boxes: impl IntoIterator<Item = &'a Rect>) -> Option<Bounds> {
    let mut iter = boxes.into_iter().peekable();
    iter.peek()?;
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for r in iter {
        x0 = x0.min(r.x);
        y0 = y0.min(r.y);
        x1 = x1.max(r.x + r.w);
        y1 = y1.max(r.y + r.h);
    }
    Some(Bounds {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    })
}
pub fn group_boxes(diagram: &Diagram, boxes: &HashMap<String, Rect>) -> Vec<GroupBox> {
    diagram
        .groups
        .iter()
        .filter_map(|g| {
            let members: Vec<&Rect> = g.node_ids.iter().filter_map(|id| boxes.get(id)).collect();
            let b = bounds(members.iter().copied())?;
            Some(GroupBox {
                group_id: g.id.clone(),
                name: g.name.clone(),
                color: crate::layout::valid_color(Some(&g.color)),
                count: members.len(),
                x: b.x - 24.0,
                y: b.y - 44.0,
                width: b.width + 48.0,
                height: b.height + 68.0,
            })
        })
        .collect()
}
/// The topmost card under a point.
pub fn hit(boxes: &HashMap<String, Rect>, p: Position, except: Option<&str>) -> Option<String> {
    let mut found: Vec<&String> = boxes
        .iter()
        .filter(|(id, r)| Some(id.as_str()) != except && r.contains(p))
        .map(|(id, _)| id)
        .collect();
    found.sort();
    found.last().map(|id| (*id).clone())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arrow {
    pub path: String,
    /// Halfway along the curve, where its label sits.
    pub label: (f64, f64),
}
/// A border-to-border arrow. `bend` pushes the middle sideways (canvas units) so parallel and
/// opposite edges between the same two cards stay distinguishable.
pub fn arrow(a: &Rect, b: &Rect, bend: f64) -> Arrow {
    let (ax, ay) = a.center();
    let (bx, by) = b.center();
    let (mx, my) = ((ax + bx) / 2.0, (ay + by) / 2.0);
    let (dx, dy) = (bx - ax, by - ay);
    let length = (dx * dx + dy * dy).sqrt().max(1.0);
    let (nx, ny) = (-dy / length, dx / length);
    let (cx, cy) = (mx + nx * bend * 2.0, my + ny * bend * 2.0);
    let (sx, sy) = a.exit(cx, cy, ARROW_GAP);
    let (tx, ty) = b.exit(cx, cy, ARROW_GAP);
    Arrow {
        path: format!("M{sx:.1},{sy:.1} Q{cx:.1},{cy:.1} {tx:.1},{ty:.1}"),
        label: (
            0.25 * sx + 0.5 * cx + 0.25 * tx,
            0.25 * sy + 0.5 * cy + 0.25 * ty,
        ),
    }
}
/// A straight preview line from a card to the pointer while dragging a new connection.
pub fn preview(a: &Rect, to: Position) -> String {
    let (sx, sy) = a.exit(to.x, to.y, 0.0);
    format!("M{sx:.1},{sy:.1} L{:.1},{:.1}", to.x, to.y)
}
/// Sideways offset for each edge: 0 for a lone edge, spread apart for edges sharing endpoints.
pub fn bends(edges: &[DiagramEdge]) -> Vec<f64> {
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut totals: HashMap<(String, String), usize> = HashMap::new();
    let key = |e: &DiagramEdge| {
        if e.source <= e.target {
            (e.source.clone(), e.target.clone())
        } else {
            (e.target.clone(), e.source.clone())
        }
    };
    for e in edges {
        *totals.entry(key(e)).or_insert(0) += 1;
    }
    edges
        .iter()
        .map(|e| {
            let k = key(e);
            let total = totals[&k];
            let index = seen.entry(k).or_insert(0);
            *index += 1;
            if total == 1 {
                return 0.0;
            }
            // Bend is relative to each edge's own direction; flip reversed edges so both sides are used.
            let offset = (*index as f64 - (total as f64 + 1.0) / 2.0) * 34.0;
            let flip = if e.source <= e.target { 1.0 } else { -1.0 };
            offset * flip
        })
        .collect()
}

/// The layout graph: one entity per node and one relation per edge, for the layout worker.
pub fn to_graph(diagram: &Diagram) -> Graph {
    Graph {
        entities: diagram
            .nodes
            .iter()
            .map(|node| Entity {
                id: node.id.clone(),
                name: node.label.clone(),
                namespace: String::new(),
                kind: "node".into(),
                fields: vec![Field::default(); if has_body(node) { 1 } else { 0 }],
                ..Entity::default()
            })
            .collect(),
        relations: diagram
            .edges
            .iter()
            .map(|edge| Relation {
                id: edge.id.clone(),
                source: edge.source.clone(),
                target: edge.target.clone(),
                label: edge.label.clone().unwrap_or_default(),
                ..Relation::default()
            })
            .collect(),
        warnings: vec![],
    }
}
pub fn to_groups(diagram: &Diagram) -> Vec<CanvasGroup> {
    diagram
        .groups
        .iter()
        .map(|g| CanvasGroup {
            id: g.id.clone(),
            name: g.name.clone(),
            node_ids: g.node_ids.clone(),
            color: Some(g.color.clone()),
        })
        .collect()
}
pub fn saved_positions(diagram: &Diagram) -> HashMap<String, Position> {
    diagram
        .layout
        .iter()
        .filter(|(_, f)| f.x.is_finite() && f.y.is_finite())
        .map(|(id, f)| (id.clone(), Position { x: f.x, y: f.y }))
        .collect()
}
/// Write positions back into the diagram's layout, keeping any stored sizes.
pub fn with_positions(diagram: &Diagram, positions: &HashMap<String, Position>) -> Diagram {
    let mut next = diagram.clone();
    for node in &diagram.nodes {
        if let Some(p) = positions.get(&node.id) {
            let frame = next.layout.entry(node.id.clone()).or_default();
            frame.x = p.x.round();
            frame.y = p.y.round();
        }
    }
    next.layout
        .retain(|id, _| diagram.nodes.iter().any(|n| &n.id == id));
    next
}
/// Positions for nodes that have none, in a column right of the existing drawing, so a node
/// added by someone else never lands on top of the user's arrangement. None when nothing is
/// placed yet, which calls for a full automatic layout instead.
pub fn place_missing(diagram: &Diagram) -> Option<HashMap<String, Position>> {
    let mut positions = saved_positions(diagram);
    if positions.is_empty() && !diagram.nodes.is_empty() {
        return None;
    }
    let right = diagram
        .nodes
        .iter()
        .filter_map(|n| positions.get(&n.id).map(|p| p.x + CARD_WIDTH))
        .fold(f64::NEG_INFINITY, f64::max);
    let top = positions
        .values()
        .map(|p| p.y)
        .fold(f64::INFINITY, f64::min);
    let mut y = if top.is_finite() { top } else { 0.0 };
    let x = if right.is_finite() { right + 90.0 } else { 0.0 };
    for node in &diagram.nodes {
        if !positions.contains_key(&node.id) {
            positions.insert(node.id.clone(), Position { x, y });
            y += card_height(node) + 40.0;
        }
    }
    Some(positions)
}
/// A fresh node id from a label: `Orders API` → `orders-api`, `orders-api-2`…
pub fn node_id(label: &str, diagram: &Diagram) -> String {
    let mut base: String = label
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while base.contains("--") {
        base = base.replace("--", "-");
    }
    let base = base.trim_matches('-');
    let base = if base.is_empty() { "node" } else { base };
    let base: String = base.chars().take(48).collect();
    let mut id = base.clone();
    let mut n = 2;
    while diagram.nodes.iter().any(|node| node.id == id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}
fn edge_id(diagram: &Diagram, source: &str, target: &str) -> String {
    let base = format!("{source}->{target}");
    let mut id = base.clone();
    let mut n = 2;
    while diagram.edges.iter().any(|e| e.id == id) {
        id = format!("{base}#{n}");
        n += 1;
    }
    id
}
/// Human label for a node kind, used when a node is created from a menu.
pub fn kind_label(kind: &str) -> String {
    match kind {
        "module" => "Module".into(),
        "service" => "Service".into(),
        "store" => "Data store".into(),
        "client" => "Client".into(),
        "external" => "External system".into(),
        "worker" => "Worker".into(),
        "queue" => "Queue".into(),
        "actor" => "User".into(),
        "note" => "Note".into(),
        other => {
            let mut chars = other.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_else(|| "Node".into())
        }
    }
}

/// One user gesture on the canvas, applied to the diagram as a pure function.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    /// A new node at a canvas point, optionally connected from an existing node.
    Add {
        at: Position,
        kind: String,
        from: Option<String>,
    },
    Connect {
        from: String,
        to: String,
    },
    Rename {
        id: String,
        label: String,
    },
    Recolor {
        ids: Vec<String>,
        color: Option<String>,
    },
    Rekind {
        ids: Vec<String>,
        kind: String,
    },
    Duplicate {
        ids: Vec<String>,
    },
    Delete {
        nodes: Vec<String>,
        edges: Vec<usize>,
    },
    EdgeLabel {
        index: usize,
        label: String,
    },
    Reverse {
        index: usize,
    },
    Layout(HashMap<String, Position>),
}
/// The edited diagram, the nodes to select afterwards, and whether meaning changed (as opposed
/// to only positions). Invalid gestures leave the diagram unchanged.
pub fn apply(diagram: &Diagram, edit: Edit) -> (Diagram, Vec<String>, bool) {
    let mut d = diagram.clone();
    match edit {
        Edit::Add { at, kind, from } => {
            let label = kind_label(&kind);
            let id = node_id(&label, &d);
            d.nodes.push(DiagramNode {
                id: id.clone(),
                kind,
                label,
                ..DiagramNode::default()
            });
            d.layout.insert(
                id.clone(),
                Frame {
                    x: at.x.round(),
                    y: at.y.round(),
                    w: None,
                    h: None,
                },
            );
            if let Some(from) = from.filter(|f| d.nodes.iter().any(|n| &n.id == f)) {
                let eid = edge_id(&d, &from, &id);
                d.edges.push(DiagramEdge {
                    id: eid,
                    source: from,
                    target: id.clone(),
                    ..DiagramEdge::default()
                });
            }
            (d, vec![id], true)
        }
        Edit::Connect { from, to } => {
            let exists = |id: &str| d.nodes.iter().any(|n| n.id == id);
            if from == to
                || !exists(&from)
                || !exists(&to)
                || d.edges.iter().any(|e| e.source == from && e.target == to)
            {
                return (d, vec![], false);
            }
            let eid = edge_id(&d, &from, &to);
            d.edges.push(DiagramEdge {
                id: eid,
                source: from,
                target: to.clone(),
                ..DiagramEdge::default()
            });
            (d, vec![to], true)
        }
        Edit::Rename { id, label } => {
            let label = label.trim().to_string();
            if let Some(node) = d
                .nodes
                .iter_mut()
                .find(|n| n.id == id)
                .filter(|_| !label.is_empty())
            {
                node.label = label.chars().take(200).collect();
            }
            (d, vec![id], true)
        }
        Edit::Recolor { ids, color } => {
            for node in d.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                let style = node.style.get_or_insert_with(Default::default);
                style.color = color.clone();
                if style.color.is_none() && style.shape.is_none() {
                    node.style = None;
                }
            }
            (d, ids, true)
        }
        Edit::Rekind { ids, kind } => {
            for node in d.nodes.iter_mut().filter(|n| ids.contains(&n.id)) {
                node.kind = kind.clone();
            }
            (d, ids, true)
        }
        Edit::Duplicate { ids } => {
            let mut created = vec![];
            let mut renamed: HashMap<String, String> = HashMap::new();
            for id in &ids {
                let Some(node) = diagram.nodes.iter().find(|n| &n.id == id) else {
                    continue;
                };
                let copy_id = node_id(&node.label, &d);
                let mut copy = node.clone();
                copy.id = copy_id.clone();
                d.nodes.push(copy);
                if let Some(frame) = diagram.layout.get(id) {
                    d.layout.insert(
                        copy_id.clone(),
                        Frame {
                            x: frame.x + 32.0,
                            y: frame.y + 32.0,
                            ..*frame
                        },
                    );
                }
                renamed.insert(id.clone(), copy_id.clone());
                created.push(copy_id);
            }
            // Edges inside the duplicated set are copied too, like copying a sub-drawing.
            for edge in &diagram.edges {
                if let (Some(s), Some(t)) = (renamed.get(&edge.source), renamed.get(&edge.target)) {
                    let eid = edge_id(&d, s, t);
                    d.edges.push(DiagramEdge {
                        id: eid,
                        source: s.clone(),
                        target: t.clone(),
                        ..edge.clone()
                    });
                }
            }
            (d, created, true)
        }
        Edit::Delete { nodes, edges } => {
            let gone: HashSet<&String> = nodes.iter().collect();
            let mut index = 0;
            d.edges.retain(|e| {
                let keep = !edges.contains(&index)
                    && !gone.contains(&e.source)
                    && !gone.contains(&e.target);
                index += 1;
                keep
            });
            d.nodes.retain(|n| !gone.contains(&n.id));
            for group in &mut d.groups {
                group.node_ids.retain(|n| !gone.contains(n));
            }
            d.groups.retain(|g| !g.node_ids.is_empty());
            d.layout.retain(|id, _| !gone.contains(id));
            let changed = d != *diagram;
            (d, vec![], changed)
        }
        Edit::EdgeLabel { index, label } => {
            if let Some(edge) = d.edges.get_mut(index) {
                let label = label.trim().to_string();
                edge.label = (!label.is_empty()).then(|| label.chars().take(200).collect());
            }
            (d, vec![], true)
        }
        Edit::Reverse { index } => {
            if let Some(edge) = d.edges.get_mut(index) {
                std::mem::swap(&mut edge.source, &mut edge.target);
                edge.id = format!("{}->{}", edge.source, edge.target);
                if diagram
                    .edges
                    .iter()
                    .enumerate()
                    .any(|(i, e)| i != index && e.id == edge.id)
                {
                    edge.id.push_str("#r");
                }
            }
            (d, vec![], true)
        }
        Edit::Layout(positions) => (with_positions(&d, &positions), vec![], false),
    }
}
pub const SWATCHES: [&str; 8] = [
    "#5b8def",
    "#55c38b",
    "#e0b04f",
    "#e0795f",
    "#b07de0",
    "#4fb8c9",
    "#8a96a3",
    DEFAULT_GROUP_COLOR,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn diagram() -> Diagram {
        Diagram {
            title: "Map".into(),
            nodes: vec![
                DiagramNode {
                    id: "a".into(),
                    label: "A".into(),
                    body_md: Some("Does things".into()),
                    ..DiagramNode::default()
                },
                DiagramNode {
                    id: "b".into(),
                    label: "B".into(),
                    ..DiagramNode::default()
                },
            ],
            edges: vec![DiagramEdge {
                id: "a->b".into(),
                source: "a".into(),
                target: "b".into(),
                ..DiagramEdge::default()
            }],
            ..Diagram::default()
        }
    }

    #[test]
    fn selection_neighborhood_keeps_only_immediate_connections() {
        let d: Diagram = serde_json::from_str(include_str!("sample-diagram.json")).unwrap();
        assert_eq!(
            neighborhood(&d, "web"),
            HashSet::from(["web".into(), "api".into()])
        );
        assert_eq!(
            neighborhood(&d, "db"),
            HashSet::from(["db".into(), "api".into(), "worker".into()])
        );
    }

    #[test]
    fn layout_graph_and_missing_nodes() {
        let mut d = diagram();
        assert_eq!(to_graph(&d).relations.len(), 1);
        assert!(card_height(&d.nodes[0]) > card_height(&d.nodes[1]));
        assert!(place_missing(&d).is_none());
        d.layout.insert(
            "a".into(),
            Frame {
                x: 10.0,
                y: 20.0,
                w: None,
                h: None,
            },
        );
        let placed = place_missing(&d).unwrap();
        assert_eq!(placed["a"], Position { x: 10.0, y: 20.0 });
        assert!(placed["b"].x >= 10.0 + CARD_WIDTH);
        assert_eq!(with_positions(&d, &placed).layout.len(), 2);
        assert_eq!(node_id("A", &d), "a-2");
    }

    #[test]
    fn diagrams_flow_left_to_right() {
        let mut d = diagram();
        d.nodes.push(DiagramNode {
            id: "c".into(),
            label: "C".into(),
            ..DiagramNode::default()
        });
        d.edges.push(DiagramEdge {
            id: "b->c".into(),
            source: "b".into(),
            target: "c".into(),
            ..DiagramEdge::default()
        });
        let p = crate::smart_layout::smart_layout(&to_graph(&d), &to_groups(&d));
        assert!(p["a"].x < p["b"].x && p["b"].x < p["c"].x, "{p:?}");
        // Neighbouring layers leave room for an arrow and its label between cards.
        assert!(p["b"].x - p["a"].x >= CARD_WIDTH + 60.0, "{p:?}");
    }

    #[test]
    fn arrows_run_border_to_border_and_parallel_edges_bend_apart() {
        let a = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let b = Rect {
            x: 300.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let straight = arrow(&a, &b, 0.0);
        assert_eq!(straight.path, "M106.0,25.0 Q200.0,25.0 294.0,25.0");
        assert_eq!(straight.label, (200.0, 25.0));
        let mut edges = diagram().edges;
        edges.push(DiagramEdge {
            id: "b->a".into(),
            source: "b".into(),
            target: "a".into(),
            ..DiagramEdge::default()
        });
        let bent = bends(&edges);
        assert!(bent[0] != 0.0 && bent[1] != 0.0);
        let first = arrow(&a, &b, bent[0]);
        let second = arrow(&b, &a, bent[1]);
        assert!(
            (first.label.1 - second.label.1).abs() > 10.0,
            "{first:?} {second:?}"
        );
        let boxes = HashMap::from([("a".to_string(), a), ("b".to_string(), b)]);
        assert_eq!(
            hit(&boxes, Position { x: 320.0, y: 10.0 }, None).as_deref(),
            Some("b")
        );
        assert_eq!(hit(&boxes, Position { x: 320.0, y: 10.0 }, Some("b")), None);
    }

    #[test]
    fn edits_are_pure_and_reject_nonsense() {
        let d = diagram();
        let (added, selected, content) = apply(
            &d,
            Edit::Add {
                at: Position { x: 5.4, y: 9.6 },
                kind: "store".into(),
                from: Some("b".into()),
            },
        );
        assert!(content);
        assert_eq!(selected, ["data-store"]);
        assert_eq!(added.layout["data-store"].x, 5.0);
        assert!(added
            .edges
            .iter()
            .any(|e| e.source == "b" && e.target == "data-store"));
        let (same, _, changed) = apply(
            &d,
            Edit::Connect {
                from: "a".into(),
                to: "a".into(),
            },
        );
        assert!(!changed && same == d);
        let (dup, ids, _) = apply(
            &d,
            Edit::Duplicate {
                ids: vec!["a".into(), "b".into()],
            },
        );
        assert_eq!(ids.len(), 2);
        assert_eq!(dup.edges.len(), 2);
        let (deleted, _, _) = apply(
            &dup,
            Edit::Delete {
                nodes: vec!["a".into()],
                edges: vec![],
            },
        );
        assert!(deleted.edges.iter().all(|e| e.source != "a"));
        let (reversed, _, _) = apply(&d, Edit::Reverse { index: 0 });
        assert_eq!(
            (
                reversed.edges[0].source.as_str(),
                reversed.edges[0].id.as_str()
            ),
            ("b", "b->a")
        );
        let (labelled, _, _) = apply(
            &d,
            Edit::EdgeLabel {
                index: 0,
                label: "  reads ".into(),
            },
        );
        assert_eq!(labelled.edges[0].label.as_deref(), Some("reads"));
        let (colored, _, _) = apply(
            &d,
            Edit::Recolor {
                ids: vec!["b".into()],
                color: Some("#5b8def".into()),
            },
        );
        assert_eq!(
            colored.nodes[1].style.as_ref().unwrap().color.as_deref(),
            Some("#5b8def")
        );
        let (plain, _, _) = apply(
            &colored,
            Edit::Recolor {
                ids: vec!["b".into()],
                color: None,
            },
        );
        assert!(plain.nodes[1].style.is_none());
    }
}
