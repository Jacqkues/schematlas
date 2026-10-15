//! Diagram geometry and editing, free of any UI. Cards are compact boxes; arrows run border to
//! border along the line between card centers, bowing apart when two nodes share several edges.
//! Canvas gestures become the parser's edits, so the meaning of an edit is defined once, by the
//! same model the backend applies to the file. Automatic layout reuses the schema worker by
//! converting a diagram to its layout graph.
use crate::layout::{Bounds, GroupBox, DEFAULT_GROUP_COLOR};
use crate::types::{
    Annotation, CanvasGroup, Diagram, DiagramEdge, DiagramEdit, DiagramNode, EdgeRef, EditTarget,
    Entity, Field, Frame, Graph, Position, Relation,
};
use schematlas_parser::model::{self, valid_id};
use std::collections::{HashMap, HashSet};

pub const CARD_WIDTH: f64 = 220.0;
pub const LABEL_HEIGHT: f64 = 64.0;
pub const BODY_HEIGHT: f64 = 44.0;
/// Members, columns or attributes listed inside a card, then "+n more".
pub const DETAIL_LINES: usize = 8;
pub const DETAIL_HEIGHT: f64 = 18.0;
/// Gap between an arrow's end and the card border, so arrowheads stay visible.
const ARROW_GAP: f64 = 6.0;
/// The annotation key for a node's accent color.
pub const COLOR: &str = "color";

pub fn note(node: &DiagramNode) -> Option<&str> {
    node.annotation
        .note
        .as_deref()
        .filter(|b| !b.trim().is_empty())
}
pub fn color(node: &DiagramNode) -> Option<String> {
    node.annotation.extra.get(COLOR).cloned()
}
/// Lines shown in the card, with a final "+n more" when they do not all fit.
pub fn detail_lines(node: &DiagramNode) -> Vec<String> {
    if node.details.len() <= DETAIL_LINES {
        return node.details.clone();
    }
    let mut lines = node.details[..DETAIL_LINES - 1].to_vec();
    lines.push(format!("+{} more", node.details.len() - (DETAIL_LINES - 1)));
    lines
}
/// The selected component and its directly connected neighbors, in either direction.
pub fn neighborhood(diagram: &Diagram, selected: &str) -> HashSet<String> {
    let mut ids = HashSet::from([selected.to_string()]);
    for edge in &diagram.edges {
        if edge.from == selected {
            ids.insert(edge.to.clone());
        }
        if edge.to == selected {
            ids.insert(edge.from.clone());
        }
    }
    ids
}

/// The label row grows past two lines, counting the lines long labels wrap to.
pub fn label_height(node: &DiagramNode) -> f64 {
    let lines = crate::label::shown_lines(&node.label);
    LABEL_HEIGHT + lines.saturating_sub(2) as f64 * crate::label::LINE_HEIGHT
}

pub fn card_height(node: &DiagramNode) -> f64 {
    let mut height = label_height(node);
    if note(node).is_some() {
        height += BODY_HEIGHT;
    }
    let details = detail_lines(node).len();
    if details > 0 {
        height += details as f64 * DETAIL_HEIGHT + 10.0;
    }
    height
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
/// Every node inside a group, including those of the groups nested in it.
pub fn group_members(diagram: &Diagram, id: &str) -> Vec<String> {
    let tree = diagram.group_tree(id);
    let mut members: Vec<String> = diagram
        .groups
        .iter()
        .filter(|g| tree.contains(&g.id))
        .flat_map(|g| g.nodes.iter().cloned())
        .collect();
    members.sort();
    members.dedup();
    members
}
pub fn group_boxes(diagram: &Diagram, boxes: &HashMap<String, Rect>) -> Vec<GroupBox> {
    // Nested groups get a little more room than what they contain.
    let depth = |id: &str| {
        let mut depth = 0;
        let mut current = diagram.groups.iter().find(|g| g.id == id);
        while let Some(parent) = current.and_then(|g| g.parent.as_deref()) {
            depth += 1;
            current = diagram.groups.iter().find(|g| g.id == parent);
        }
        depth as f64
    };
    let deepest = diagram
        .groups
        .iter()
        .map(|g| depth(&g.id))
        .fold(0.0, f64::max);
    diagram
        .groups
        .iter()
        .filter_map(|g| {
            let members = group_members(diagram, &g.id);
            let found: Vec<&Rect> = members.iter().filter_map(|id| boxes.get(id)).collect();
            let b = bounds(found.iter().copied())?;
            let pad = 24.0 + (deepest - depth(&g.id)) * 16.0;
            Some(GroupBox {
                group_id: g.id.clone(),
                name: g.title.clone(),
                color: DEFAULT_GROUP_COLOR.into(),
                count: found.len(),
                x: b.x - pad,
                y: b.y - pad - 20.0,
                width: b.width + pad * 2.0,
                height: b.height + pad * 2.0 + 20.0,
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
    /// Near each end, where cardinalities and role names sit.
    pub tail: (f64, f64),
    pub head: (f64, f64),
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
    // Beside the line near each end, offset to the left of its direction so the text clears it.
    let near = |(px, py): (f64, f64), toward: (f64, f64)| {
        let (vx, vy) = (toward.0 - px, toward.1 - py);
        let len = (vx * vx + vy * vy).sqrt().max(1.0);
        let along = 22.0_f64.min(len / 3.0);
        (
            px + vx / len * along + vy / len * 11.0,
            py + vy / len * along - vx / len * 11.0,
        )
    };
    Arrow {
        path: format!("M{sx:.1},{sy:.1} Q{cx:.1},{cy:.1} {tx:.1},{ty:.1}"),
        label: (
            0.25 * sx + 0.5 * cx + 0.25 * tx,
            0.25 * sy + 0.5 * cy + 0.25 * ty,
        ),
        tail: near((sx, sy), (cx, cy)),
        head: near((tx, ty), (cx, cy)),
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
        if e.from <= e.to {
            (e.from.clone(), e.to.clone())
        } else {
            (e.to.clone(), e.from.clone())
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
            let flip = if e.from <= e.to { 1.0 } else { -1.0 };
            offset * flip
        })
        .collect()
}

/// The layout graph: one entity per node and one relation per edge between nodes, for the
/// layout worker. Edges to groups have no card to route to and are left out.
pub fn to_graph(diagram: &Diagram) -> Graph {
    let ids: HashSet<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
    Graph {
        entities: diagram
            .nodes
            .iter()
            .map(|node| {
                // The worker sizes boxes by field count; one field per extra row of the card.
                let rows = ((card_height(node) - LABEL_HEIGHT) / DETAIL_HEIGHT).ceil() as usize;
                Entity {
                    id: node.id.clone(),
                    name: node.label.clone(),
                    namespace: String::new(),
                    kind: "node".into(),
                    fields: vec![Field::default(); rows],
                    ..Entity::default()
                }
            })
            .collect(),
        relations: diagram
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| ids.contains(e.from.as_str()) && ids.contains(e.to.as_str()))
            .map(|(i, edge)| Relation {
                id: format!("e{i}"),
                source: edge.from.clone(),
                target: edge.to.clone(),
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
            name: g.title.clone(),
            node_ids: group_members(diagram, &g.id),
            color: None,
        })
        .collect()
}
/// Stored positions of nodes (groups are drawn around their members).
pub fn saved_positions(diagram: &Diagram) -> HashMap<String, Position> {
    diagram
        .nodes
        .iter()
        .filter_map(|n| {
            let f = diagram.layout.get(&n.id)?;
            (f.x.is_finite() && f.y.is_finite())
                .then(|| (n.id.clone(), Position { x: f.x, y: f.y }))
        })
        .collect()
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
/// A fresh node id from a label that every format can write unquoted: `Orders API` →
/// `orders-api`, `orders-api-2`…
pub fn node_id(label: &str, diagram: &Diagram) -> String {
    let mut base: String = crate::label::plain(label)
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
    let mut base: String = base.chars().take(48).collect();
    if !valid_id(&base) {
        base.push_str("-node");
    }
    let taken = |id: &str| {
        diagram.nodes.iter().any(|n| n.id == id) || diagram.groups.iter().any(|g| g.id == id)
    };
    let mut id = base.clone();
    let mut n = 2;
    while taken(&id) {
        id = format!("{base}-{n}");
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
/// The shape a new node of this kind is drawn with in the file, so the kind reads in any
/// Mermaid or draw.io viewer, not only here.
pub fn kind_shape(kind: &str) -> Option<String> {
    match kind {
        "store" => Some("cylinder".into()),
        "decision" => Some("diamond".into()),
        "actor" => Some("circle".into()),
        "queue" | "worker" => Some("subroutine".into()),
        _ => None,
    }
}

/// One user gesture on the canvas.
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
    /// Replace a node's annotation: its kind, note, code references, tags and color.
    Annotate {
        id: String,
        annotation: Annotation,
    },
    /// Replace the diagram's own annotation (its title).
    AnnotateDiagram(Annotation),
    Layout(HashMap<String, Position>),
}
impl Edit {
    /// Whether the gesture changes only annotations or positions, which every editable file
    /// takes; the rest changes the structure written in the file.
    pub fn annotation_only(&self) -> bool {
        matches!(
            self,
            Edit::Recolor { .. }
                | Edit::Rekind { .. }
                | Edit::Annotate { .. }
                | Edit::AnnotateDiagram(_)
                | Edit::Layout(_)
        )
    }
}

/// A gesture as the parser's edits, with the diagram they lead to and the nodes to select.
#[derive(Clone, Debug, PartialEq)]
pub struct Planned {
    pub edits: Vec<DiagramEdit>,
    pub diagram: Diagram,
    pub select: Vec<String>,
}

/// Apply one edit to the model as the backend will. Positions of formats that keep them beside
/// the file are set directly.
pub fn apply_edit(diagram: &Diagram, edit: &DiagramEdit) -> Result<Diagram, String> {
    if let DiagramEdit::Move { id, x, y } = edit {
        if diagram.kind != model::DRAWIO {
            let mut next = diagram.clone();
            let size = next.layout.get(id).copied().unwrap_or(Frame {
                x: 0.0,
                y: 0.0,
                width: model::NEW_NODE_SIZE.0,
                height: model::NEW_NODE_SIZE.1,
            });
            next.layout.insert(
                id.clone(),
                Frame {
                    x: *x,
                    y: *y,
                    ..size
                },
            );
            return Ok(next);
        }
    }
    model::apply(diagram, edit).map_err(|e| e.to_string())
}

struct Builder {
    diagram: Diagram,
    edits: Vec<DiagramEdit>,
}
impl Builder {
    fn push(&mut self, edit: DiagramEdit) -> Result<(), String> {
        self.diagram = apply_edit(&self.diagram, &edit)?;
        self.edits.push(edit);
        Ok(())
    }
    fn node(&self, id: &str) -> Option<&DiagramNode> {
        self.diagram.nodes.iter().find(|n| n.id == id)
    }
    fn annotate(&mut self, id: &str, change: impl Fn(&mut Annotation)) -> Result<(), String> {
        let Some(node) = self.node(id) else {
            return Ok(());
        };
        let mut annotation = node.annotation.clone();
        change(&mut annotation);
        if annotation == node.annotation {
            return Ok(());
        }
        self.push(DiagramEdit::Annotate {
            target: EditTarget::Node(id.to_owned()),
            annotation,
        })
    }
    fn place(&mut self, id: &str, at: Position) -> Result<(), String> {
        self.push(DiagramEdit::Move {
            id: id.to_owned(),
            x: at.x.round(),
            y: at.y.round(),
        })
    }
    /// A new node of `kind`, its kind recorded only when the shape does not already say it.
    fn add(&mut self, label: &str, kind: &str, at: Position) -> Result<String, String> {
        let id = node_id(label, &self.diagram);
        self.push(DiagramEdit::AddNode {
            id: id.clone(),
            label: label.to_owned(),
            shape: kind_shape(kind),
        })?;
        let derived = self.node(&id).map(|n| n.kind().to_owned());
        if derived.as_deref() != Some(kind) {
            self.annotate(&id, |a| a.kind = Some(kind.to_owned()))?;
        }
        self.place(&id, at)?;
        Ok(id)
    }
}

/// Plan a gesture. Gestures that would change nothing, or that the model refuses, are errors
/// with a reason to show.
pub fn plan(diagram: &Diagram, edit: Edit) -> Result<Planned, String> {
    let mut b = Builder {
        diagram: diagram.clone(),
        edits: vec![],
    };
    let mut select = vec![];
    match edit {
        Edit::Add { at, kind, from } => {
            let id = b.add(&kind_label(&kind), &kind, at)?;
            if let Some(from) = from.filter(|f| diagram.nodes.iter().any(|n| &n.id == f)) {
                b.push(DiagramEdit::AddEdge {
                    from,
                    to: id.clone(),
                    label: None,
                })?;
            }
            select.push(id);
        }
        Edit::Connect { from, to } => {
            if from == to || diagram.edges.iter().any(|e| e.from == from && e.to == to) {
                return Err("These are already connected.".into());
            }
            b.push(DiagramEdit::AddEdge {
                from,
                to: to.clone(),
                label: None,
            })?;
            select.push(to);
        }
        Edit::Rename { id, label } => {
            let label: String = label.trim().chars().take(200).collect();
            if !label.is_empty() && b.node(&id).is_some_and(|n| n.label != label) {
                b.push(DiagramEdit::SetLabel {
                    id: id.clone(),
                    label,
                })?;
            }
            select.push(id);
        }
        Edit::Recolor { ids, color } => {
            for id in &ids {
                b.annotate(id, |a| match &color {
                    Some(c) => {
                        a.extra.insert(COLOR.into(), c.clone());
                    }
                    None => {
                        a.extra.remove(COLOR);
                    }
                })?;
            }
            select = ids;
        }
        Edit::Rekind { ids, kind } => {
            for id in &ids {
                b.annotate(id, |a| a.kind = Some(kind.clone()))?;
            }
            select = ids;
        }
        Edit::Duplicate { ids } => {
            let mut renamed: HashMap<String, String> = HashMap::new();
            for id in &ids {
                let Some(node) = diagram.nodes.iter().find(|n| &n.id == id) else {
                    continue;
                };
                let copy = node_id(&node.label, &b.diagram);
                b.push(DiagramEdit::AddNode {
                    id: copy.clone(),
                    label: node.label.clone(),
                    shape: node.shape.clone(),
                })?;
                if !node.annotation.is_empty() {
                    b.push(DiagramEdit::Annotate {
                        target: EditTarget::Node(copy.clone()),
                        annotation: node.annotation.clone(),
                    })?;
                }
                if let Some(frame) = diagram.layout.get(id) {
                    b.place(
                        &copy,
                        Position {
                            x: frame.x + 32.0,
                            y: frame.y + 32.0,
                        },
                    )?;
                }
                renamed.insert(id.clone(), copy.clone());
                select.push(copy);
            }
            // Edges inside the duplicated set are copied too, like copying a sub-drawing.
            for edge in &diagram.edges {
                if let (Some(from), Some(to)) = (renamed.get(&edge.from), renamed.get(&edge.to)) {
                    b.push(DiagramEdit::AddEdge {
                        from: from.clone(),
                        to: to.clone(),
                        label: edge.label.clone(),
                    })?;
                }
            }
        }
        Edit::Delete { nodes, edges } => {
            // Later edges first, so each reference still points at the edge meant.
            let mut edges = edges;
            edges.sort_unstable();
            edges.dedup();
            for index in edges.into_iter().rev() {
                let Some(edge) = diagram.edges.get(index) else {
                    continue;
                };
                if nodes.contains(&edge.from) || nodes.contains(&edge.to) {
                    continue;
                }
                b.push(DiagramEdit::RemoveEdge(EdgeRef::of(&b.diagram, index)))?;
            }
            for id in &nodes {
                if b.node(id).is_some() {
                    b.push(DiagramEdit::RemoveNode { id: id.clone() })?;
                }
            }
        }
        Edit::EdgeLabel { index, label } => {
            let edge = diagram.edges.get(index).ok_or("No such connection.")?;
            let label: String = label.trim().chars().take(200).collect();
            let label = (!label.is_empty()).then_some(label);
            if edge.label != label {
                b.push(DiagramEdit::SetEdgeLabel {
                    edge: EdgeRef::of(diagram, index),
                    label,
                })?;
            }
        }
        Edit::Reverse { index } => {
            let edge = diagram.edges.get(index).ok_or("No such connection.")?;
            b.push(DiagramEdit::RemoveEdge(EdgeRef::of(diagram, index)))?;
            b.push(DiagramEdit::AddEdge {
                from: edge.to.clone(),
                to: edge.from.clone(),
                label: edge.label.clone(),
            })?;
            if !edge.annotation.is_empty() {
                let last = b.diagram.edges.len() - 1;
                b.push(DiagramEdit::Annotate {
                    target: EditTarget::Edge(EdgeRef::of(&b.diagram, last)),
                    annotation: edge.annotation.clone(),
                })?;
            }
        }
        Edit::Annotate { id, annotation } => {
            b.annotate(&id, |a| *a = annotation.clone())?;
            select.push(id);
        }
        Edit::AnnotateDiagram(annotation) => {
            if annotation != diagram.annotation {
                b.push(DiagramEdit::Annotate {
                    target: EditTarget::Diagram,
                    annotation,
                })?;
            }
        }
        Edit::Layout(positions) => {
            let saved = saved_positions(diagram);
            let mut ids: Vec<&String> = positions.keys().collect();
            ids.sort();
            for id in ids {
                let p = positions[id];
                let at = Position {
                    x: p.x.round(),
                    y: p.y.round(),
                };
                if b.node(id).is_some() && saved.get(id) != Some(&at) {
                    b.place(id, at)?;
                }
            }
        }
    }
    Ok(Planned {
        edits: b.edits,
        diagram: b.diagram,
        select,
    })
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
    use crate::types::CodeRef;

    fn sample() -> Diagram {
        serde_json::from_str(include_str!("sample-diagram.json")).unwrap()
    }
    fn diagram() -> Diagram {
        let mut d = Diagram {
            kind: "flowchart".into(),
            nodes: vec![
                DiagramNode {
                    id: "a".into(),
                    label: "A".into(),
                    ..DiagramNode::default()
                },
                DiagramNode {
                    id: "b".into(),
                    label: "B".into(),
                    ..DiagramNode::default()
                },
            ],
            edges: vec![DiagramEdge {
                from: "a".into(),
                to: "b".into(),
                ..DiagramEdge::default()
            }],
            ..Diagram::default()
        };
        d.nodes[0].annotation.note = Some("Does things".into());
        d
    }

    #[test]
    fn the_sample_reads_and_keeps_its_annotations() {
        let d = sample();
        assert_eq!(d.title.as_deref(), Some("Northstar architecture"));
        let web = &d.nodes[0];
        assert_eq!(web.kind(), "client");
        assert_eq!(
            web.annotation.code[0].to_string(),
            "web/app/checkout/page.tsx:1-80"
        );
        let db = d.nodes.iter().find(|n| n.id == "db").unwrap();
        assert_eq!(db.kind(), "store");
        assert_eq!(group_members(&d, "backend"), ["api", "db", "worker"]);
    }

    #[test]
    fn selection_neighborhood_keeps_only_immediate_connections() {
        let d = sample();
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
    fn cards_grow_with_notes_and_details() {
        let mut d = diagram();
        assert!(card_height(&d.nodes[0]) > card_height(&d.nodes[1]));
        d.nodes[1].details = (0..20).map(|i| format!("col{i} int")).collect();
        let lines = detail_lines(&d.nodes[1]);
        assert_eq!(lines.len(), DETAIL_LINES);
        assert_eq!(lines.last().unwrap(), "+13 more");
        assert!(card_height(&d.nodes[1]) > card_height(&d.nodes[0]));
        let g = to_graph(&d);
        assert!(g.entities[1].fields.len() > g.entities[0].fields.len());
    }

    #[test]
    fn layout_graph_and_missing_nodes() {
        let mut d = diagram();
        assert_eq!(to_graph(&d).relations.len(), 1);
        assert!(place_missing(&d).is_none());
        d.layout.insert(
            "a".into(),
            Frame {
                x: 10.0,
                y: 20.0,
                width: 120.0,
                height: 60.0,
            },
        );
        let placed = place_missing(&d).unwrap();
        assert_eq!(placed["a"], Position { x: 10.0, y: 20.0 });
        assert!(placed["b"].x >= 10.0 + CARD_WIDTH);
        assert_eq!(node_id("A", &d), "a-2");
        assert_eq!(node_id("end", &d), "end-node");
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
            from: "b".into(),
            to: "c".into(),
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
            from: "b".into(),
            to: "a".into(),
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
    fn hand_written_structures_take_only_annotations_and_layout() {
        assert!(Edit::Layout(HashMap::new()).annotation_only());
        assert!(Edit::Rekind {
            ids: vec![],
            kind: "service".into()
        }
        .annotation_only());
        assert!(!Edit::Rename {
            id: "a".into(),
            label: "A".into()
        }
        .annotation_only());
        assert!(!Edit::Delete {
            nodes: vec![],
            edges: vec![0]
        }
        .annotation_only());
    }

    #[test]
    fn gestures_become_parser_edits() {
        let d = diagram();
        let added = plan(
            &d,
            Edit::Add {
                at: Position { x: 5.4, y: 9.6 },
                kind: "store".into(),
                from: Some("b".into()),
            },
        )
        .unwrap();
        assert_eq!(added.select, ["data-store"]);
        // The shape says "store", so no annotation is needed for the kind.
        assert_eq!(
            added.edits,
            [
                DiagramEdit::AddNode {
                    id: "data-store".into(),
                    label: "Data store".into(),
                    shape: Some("cylinder".into()),
                },
                DiagramEdit::Move {
                    id: "data-store".into(),
                    x: 5.0,
                    y: 10.0,
                },
                DiagramEdit::AddEdge {
                    from: "b".into(),
                    to: "data-store".into(),
                    label: None,
                },
            ]
        );
        assert_eq!(added.diagram.layout["data-store"].x, 5.0);
        let service = plan(
            &d,
            Edit::Add {
                at: Position { x: 0.0, y: 0.0 },
                kind: "service".into(),
                from: None,
            },
        )
        .unwrap();
        assert_eq!(service.diagram.nodes[2].kind(), "service");
        assert!(plan(
            &d,
            Edit::Connect {
                from: "a".into(),
                to: "b".into(),
            },
        )
        .is_err());
        let dup = plan(
            &d,
            Edit::Duplicate {
                ids: vec!["a".into(), "b".into()],
            },
        )
        .unwrap();
        assert_eq!(dup.select, ["a-2", "b-2"]);
        assert_eq!(dup.diagram.edges.len(), 2);
        assert_eq!(
            dup.diagram.nodes[2].annotation.note.as_deref(),
            Some("Does things")
        );
        let deleted = plan(
            &dup.diagram,
            Edit::Delete {
                nodes: vec!["a".into()],
                edges: vec![1, 0],
            },
        )
        .unwrap();
        // The edge from `a` goes with it; only the other selected edge is removed by reference.
        assert_eq!(
            deleted.edits,
            [
                DiagramEdit::RemoveEdge(EdgeRef {
                    from: "a-2".into(),
                    to: "b-2".into(),
                    nth: 0
                }),
                DiagramEdit::RemoveNode { id: "a".into() },
            ]
        );
        assert!(deleted.diagram.edges.is_empty());
        let reversed = plan(&d, Edit::Reverse { index: 0 }).unwrap();
        assert_eq!(
            (
                reversed.diagram.edges[0].from.as_str(),
                reversed.diagram.edges[0].to.as_str()
            ),
            ("b", "a")
        );
        let labelled = plan(
            &d,
            Edit::EdgeLabel {
                index: 0,
                label: "  reads ".into(),
            },
        )
        .unwrap();
        assert_eq!(labelled.diagram.edges[0].label.as_deref(), Some("reads"));
        let colored = plan(
            &d,
            Edit::Recolor {
                ids: vec!["b".into()],
                color: Some("#5b8def".into()),
            },
        )
        .unwrap();
        assert_eq!(color(&colored.diagram.nodes[1]).as_deref(), Some("#5b8def"));
        let plain = plan(
            &colored.diagram,
            Edit::Recolor {
                ids: vec!["b".into()],
                color: None,
            },
        )
        .unwrap();
        assert!(plain.diagram.nodes[1].annotation.is_empty());
        // Unchanged positions are not written again.
        let mut placed = d.clone();
        placed = apply_edit(
            &placed,
            &DiagramEdit::Move {
                id: "a".into(),
                x: 1.0,
                y: 2.0,
            },
        )
        .unwrap();
        let moved = plan(
            &placed,
            Edit::Layout(HashMap::from([
                ("a".to_string(), Position { x: 1.2, y: 2.0 }),
                ("b".to_string(), Position { x: 300.0, y: 2.0 }),
            ])),
        )
        .unwrap();
        assert_eq!(moved.edits.len(), 1);
        // Bad code references are refused before anything is sent.
        let mut annotation = Annotation::default();
        annotation.code.push(CodeRef::parse("../secret"));
        assert!(plan(
            &d,
            Edit::Annotate {
                id: "a".into(),
                annotation,
            },
        )
        .is_err());
    }
}
