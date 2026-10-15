//! Automatic diagram layout with dugong, a port of dagre (the layout engine Mermaid uses), so a
//! diagram drawn here reads like the same file rendered by Mermaid: ranks follow the file's
//! direction, subgraphs become boxes around their members, and every edge gets a route that
//! goes around the cards instead of through them. Pure: it runs in the layout worker.
use crate::diagram_layout::{card_height, group_members, CARD_WIDTH};
use crate::types::{Diagram, Position};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, RankDir};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const NODE: &str = "n:";
const GROUP: &str = "g:";
/// Room inside the top of a group box for its title.
pub const GROUP_TITLE: f64 = 30.0;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: String,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    /// Size of the label drawn on the edge, so ranks leave room for it.
    pub label: Option<(f64, f64)>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowGroup {
    pub id: String,
    pub parent: Option<String>,
    /// Direct member nodes.
    pub nodes: Vec<String>,
}
/// What the layout needs from a diagram, small enough to post to the worker.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowInput {
    /// Mermaid's `TB`, `TD`, `BT`, `LR` or `RL`.
    pub direction: String,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub groups: Vec<FlowGroup>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Box2 {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// From the source card's border to the target card's border.
    pub points: Vec<(f64, f64)>,
    /// Where the edge's label is centered.
    pub label: Option<(f64, f64)>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowLayout {
    /// Top-left corner of every card.
    pub nodes: HashMap<String, Position>,
    /// One per input edge, in order; None for edges the layout cannot route (to a group).
    pub routes: Vec<Option<Route>>,
    pub groups: HashMap<String, Box2>,
}

/// What the worker is asked to lay out.
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Job {
    /// Database tables and API operations, arranged by domain.
    Schema(crate::types::Graph, Vec<crate::types::CanvasGroup>),
    /// A diagram, laid out like Mermaid draws it.
    Flow(FlowInput),
}
impl Job {
    /// The worker's answer as JSON: `Result<positions, error>` or `Result<FlowLayout, error>`.
    pub fn run(self) -> String {
        let json = match self {
            Job::Schema(graph, groups) => serde_json::to_string(&Ok::<_, String>(
                crate::smart_layout::smart_layout(&graph, &groups),
            )),
            Job::Flow(input) => serde_json::to_string(&layout(&input)),
        };
        json.unwrap_or_else(|e| format!("{{\"Err\":{:?}}}", e.to_string()))
    }
}

/// Approximate width of a short label in the canvas's 10px monospace.
fn label_size(text: &str) -> (f64, f64) {
    let chars = text.chars().count().min(40) as f64;
    (chars * 6.2 + 18.0, 22.0)
}

/// Mermaid's default for each family when the file names no direction.
fn direction(diagram: &Diagram) -> String {
    diagram.direction.clone().unwrap_or_else(|| {
        match diagram.kind.as_str() {
            "er" | "dbml" => "LR",
            _ => "TB",
        }
        .into()
    })
}

/// The layout input for a diagram, with the canvas's real card sizes.
pub fn input(diagram: &Diagram) -> FlowInput {
    let nodes: HashSet<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
    FlowInput {
        direction: direction(diagram),
        nodes: diagram
            .nodes
            .iter()
            .map(|n| FlowNode {
                id: n.id.clone(),
                width: CARD_WIDTH,
                height: card_height(n),
            })
            .collect(),
        edges: diagram
            .edges
            .iter()
            .map(|e| FlowEdge {
                from: e.from.clone(),
                to: e.to.clone(),
                label: e.label.as_deref().filter(|l| !l.is_empty()).map(label_size),
            })
            .collect(),
        groups: diagram
            .groups
            .iter()
            .filter(|g| !group_members(diagram, &g.id).is_empty())
            .map(|g| crate::flow_layout::FlowGroup {
                id: g.id.clone(),
                parent: g.parent.clone(),
                nodes: g
                    .nodes
                    .iter()
                    .filter(|n| nodes.contains(n.as_str()))
                    .cloned()
                    .collect(),
            })
            .collect(),
    }
}

/// Lay out a diagram. Deterministic: the same input always gives the same picture.
///
/// Dagre's compound layout (subgraphs drawn around their members, as Mermaid does) comes first.
/// When it lets a card overlap or an edge cut through a card, which dagre can do with groups
/// linked to many outside nodes, a hierarchical layout takes over: each group is laid out on its
/// own and placed as one block, so nothing can end up inside a group it does not belong to.
pub fn layout(input: &FlowInput) -> Result<FlowLayout, String> {
    if input.nodes.is_empty() {
        return Ok(FlowLayout::default());
    }
    let first = compound(input)?;
    let issues = problems(input, &first);
    if issues == 0 || input.groups.is_empty() {
        return Ok(first);
    }
    let second = hierarchical(input)?;
    Ok(if problems(input, &second) < issues {
        second
    } else {
        first
    })
}

fn rankdir(direction: &str) -> (RankDir, bool) {
    let rankdir = match direction.to_ascii_uppercase().as_str() {
        "LR" => RankDir::LR,
        "RL" => RankDir::RL,
        "BT" => RankDir::BT,
        _ => RankDir::TB,
    };
    (rankdir, matches!(rankdir, RankDir::LR | RankDir::RL))
}
fn new_graph(direction: &str, compound: bool) -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, bool) {
    let (rankdir, horizontal) = rankdir(direction);
    let mut g = Graph::new(GraphOptions {
        multigraph: true,
        compound,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir,
        nodesep: if horizontal { 44.0 } else { 56.0 },
        ranksep: if horizontal { 90.0 } else { 70.0 },
        edgesep: 24.0,
        ..GraphLabel::default()
    });
    (g, horizontal)
}
fn edge_label(label: Option<(f64, f64)>) -> EdgeLabel {
    let (width, height) = label.unwrap_or((0.0, 0.0));
    EdgeLabel {
        width,
        height,
        minlen: 1,
        weight: 1.0,
        ..EdgeLabel::default()
    }
}

/// Dagre's own compound layout of the whole diagram.
fn compound(input: &FlowInput) -> Result<FlowLayout, String> {
    let (mut g, horizontal) = new_graph(&input.direction, true);
    let known: HashSet<&str> = input.nodes.iter().map(|n| n.id.as_str()).collect();
    for n in &input.nodes {
        g.set_node(
            format!("{NODE}{}", n.id),
            NodeLabel {
                width: n.width,
                height: n.height,
                ..NodeLabel::default()
            },
        );
    }
    let groups: HashSet<&str> = input.groups.iter().map(|g| g.id.as_str()).collect();
    for group in &input.groups {
        g.set_node(format!("{GROUP}{}", group.id), NodeLabel::default());
    }
    for group in &input.groups {
        if let Some(parent) = group.parent.as_deref().filter(|p| groups.contains(p)) {
            g.set_parent(format!("{GROUP}{}", group.id), format!("{GROUP}{parent}"));
        }
        for member in group.nodes.iter().filter(|m| known.contains(m.as_str())) {
            g.set_parent(format!("{NODE}{member}"), format!("{GROUP}{}", group.id));
        }
    }
    for (i, e) in input.edges.iter().enumerate() {
        if !known.contains(e.from.as_str()) || !known.contains(e.to.as_str()) {
            continue;
        }
        g.set_edge_named(
            format!("{NODE}{}", e.from),
            format!("{NODE}{}", e.to),
            Some(i.to_string()),
            Some(edge_label(e.label)),
        );
    }
    dugong::layout(&mut g).map_err(|e| format!("Automatic layout failed: {e:?}"))?;

    let mut out = FlowLayout::default();
    for n in &input.nodes {
        let label = g
            .node(&format!("{NODE}{}", n.id))
            .ok_or("A node was lost")?;
        let (x, y) = (label.x.unwrap_or(0.0), label.y.unwrap_or(0.0));
        out.nodes.insert(
            n.id.clone(),
            Position {
                x: x - n.width / 2.0,
                y: y - n.height / 2.0,
            },
        );
    }
    for group in &input.groups {
        let Some(label) = g.node(&format!("{GROUP}{}", group.id)) else {
            continue;
        };
        let (x, y) = (label.x.unwrap_or(0.0), label.y.unwrap_or(0.0));
        out.groups.insert(
            group.id.clone(),
            Box2 {
                x: x - label.width / 2.0,
                y: y - label.height / 2.0,
                width: label.width,
                height: label.height,
            },
        );
    }
    let sizes: HashMap<&str, (f64, f64)> = input
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), (n.width, n.height)))
        .collect();
    let card = |id: &str| -> Option<Card> {
        let p = out.nodes.get(id)?;
        let (w, h) = sizes.get(id)?;
        Some(Card {
            x: p.x,
            y: p.y,
            w: *w,
            h: *h,
        })
    };
    // Each rank's extent along the rank axis: cards of different sizes share a rank line.
    let bands = Bands::new(input.nodes.iter().filter_map(|n| card(&n.id)).map(|c| {
        let (center, size) = if horizontal {
            (c.x + c.w / 2.0, c.w)
        } else {
            (c.y + c.h / 2.0, c.h)
        };
        (center, size)
    }));
    out.routes = input
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let label = g.edge(
                &format!("{NODE}{}", e.from),
                &format!("{NODE}{}", e.to),
                Some(&i.to_string()),
            )?;
            if label.points.len() < 2 {
                return None;
            }
            let raw: Vec<(f64, f64)> = label.points.iter().map(|p| (p.x, p.y)).collect();
            let points = if e.from == e.to {
                raw
            } else {
                attach(&raw, card(&e.from)?, card(&e.to)?, horizontal, &bands)
            };
            Some(Route {
                points,
                label: e
                    .label
                    .and(label.x.zip(label.y))
                    .or_else(|| middle(&label.points)),
            })
        })
        .collect();
    shift_to_origin(&mut out);
    Ok(out)
}

/// Room between a group's box and what it contains.
const PAD: f64 = 20.0;

/// A route's points and where its label sits.
type Path = (Vec<(f64, f64)>, Option<(f64, f64)>);

/// One group's contents laid out on their own, in the block's own coordinates.
#[derive(Default)]
struct Block {
    nodes: HashMap<String, Position>,
    groups: HashMap<String, Box2>,
    /// Routes of edges whose two ends are both inside the block.
    routes: HashMap<usize, Path>,
    width: f64,
    height: f64,
}
impl Block {
    fn shift(&mut self, dx: f64, dy: f64) {
        for p in self.nodes.values_mut() {
            p.x += dx;
            p.y += dy;
        }
        for b in self.groups.values_mut() {
            b.x += dx;
            b.y += dy;
        }
        for (points, label) in self.routes.values_mut() {
            for p in points.iter_mut() {
                p.0 += dx;
                p.1 += dy;
            }
            if let Some(l) = label {
                l.0 += dx;
                l.1 += dy;
            }
        }
    }
}

/// Where groups sit in each other.
struct Tree<'a> {
    input: &'a FlowInput,
    /// Innermost group of each node.
    home: HashMap<&'a str, &'a str>,
    parent: HashMap<&'a str, &'a str>,
    sizes: HashMap<&'a str, (f64, f64)>,
}
impl<'a> Tree<'a> {
    fn new(input: &'a FlowInput) -> Self {
        let ids: HashSet<&str> = input.groups.iter().map(|g| g.id.as_str()).collect();
        let mut parent = HashMap::new();
        for g in &input.groups {
            if let Some(p) = g
                .parent
                .as_deref()
                .filter(|p| ids.contains(p) && *p != g.id)
            {
                parent.insert(g.id.as_str(), p);
            }
        }
        // A cycle of parents would never end: such groups become top-level.
        let looping: Vec<&str> = parent
            .keys()
            .copied()
            .filter(|g| {
                let mut seen = HashSet::from([*g]);
                let mut at = *g;
                while let Some(p) = parent.get(at) {
                    if !seen.insert(*p) {
                        return true;
                    }
                    at = p;
                }
                false
            })
            .collect();
        for g in looping {
            parent.remove(g);
        }
        let mut home = HashMap::new();
        for g in &input.groups {
            for n in &g.nodes {
                home.entry(n.as_str()).or_insert(g.id.as_str());
            }
        }
        Tree {
            input,
            home,
            parent,
            sizes: input
                .nodes
                .iter()
                .map(|n| (n.id.as_str(), (n.width, n.height)))
                .collect(),
        }
    }
    /// The node's groups, innermost first.
    fn chain(&self, node: &str) -> Vec<&'a str> {
        let mut out = vec![];
        let mut at = self.home.get(node).copied();
        while let Some(g) = at {
            out.push(g);
            at = self.parent.get(g).copied();
        }
        out
    }
    /// What stands for `node` among `level`'s direct contents, or None if it is outside.
    fn rep(&self, level: Option<&str>, node: &str) -> Option<String> {
        let chain = self.chain(node);
        let k = match level {
            None => chain.len(),
            Some(l) => chain.iter().position(|g| *g == l)?,
        };
        Some(if k == 0 {
            format!("{NODE}{node}")
        } else {
            format!("{GROUP}{}", chain[k - 1])
        })
    }
    fn children(&self, level: Option<&str>) -> Vec<&'a str> {
        self.input
            .groups
            .iter()
            .map(|g| g.id.as_str())
            .filter(|g| self.parent.get(g).copied() == level)
            .collect()
    }
    fn card(&self, block: &Block, node: &str) -> Option<Card> {
        let p = block.nodes.get(node)?;
        let (w, h) = self.sizes.get(node)?;
        Some(Card {
            x: p.x,
            y: p.y,
            w: *w,
            h: *h,
        })
    }
    fn block(&self, level: Option<&str>) -> Result<Block, String> {
        let (mut g, horizontal) = new_graph(&self.input.direction, false);
        let mut inner: Vec<(&str, Block)> = vec![];
        let mut sizes: HashMap<String, (f64, f64)> = HashMap::new();
        for child in self.children(level) {
            let block = self.block(Some(child))?;
            if block.nodes.is_empty() {
                continue;
            }
            let size = (
                block.width + 2.0 * PAD,
                block.height + 2.0 * PAD + GROUP_TITLE,
            );
            sizes.insert(format!("{GROUP}{child}"), size);
            inner.push((child, block));
        }
        for n in &self.input.nodes {
            let id = format!("{NODE}{}", n.id);
            if self.rep(level, &n.id).as_deref() == Some(id.as_str()) {
                sizes.insert(id, (n.width, n.height));
            }
        }
        for (id, (w, h)) in &sizes {
            g.set_node(
                id.clone(),
                NodeLabel {
                    width: *w,
                    height: *h,
                    ..NodeLabel::default()
                },
            );
        }
        // Edges between two different things at this level; deeper ends are reached later.
        let mut edges: Vec<(usize, String, String)> = vec![];
        for (i, e) in self.input.edges.iter().enumerate() {
            let (Some(a), Some(b)) = (self.rep(level, &e.from), self.rep(level, &e.to)) else {
                continue;
            };
            if a == b && a.starts_with(GROUP) {
                continue;
            }
            g.set_edge_named(&a, &b, Some(i.to_string()), Some(edge_label(e.label)));
            edges.push((i, a, b));
        }
        dugong::layout(&mut g).map_err(|e| format!("Automatic layout failed: {e:?}"))?;
        let cards: HashMap<String, Card> = sizes
            .iter()
            .filter_map(|(id, (w, h))| {
                let l = g.node(id)?;
                Some((
                    id.clone(),
                    Card {
                        x: l.x.unwrap_or(0.0) - w / 2.0,
                        y: l.y.unwrap_or(0.0) - h / 2.0,
                        w: *w,
                        h: *h,
                    },
                ))
            })
            .collect();
        let bands = Bands::new(cards.values().map(|c| {
            if horizontal {
                (c.x + c.w / 2.0, c.w)
            } else {
                (c.y + c.h / 2.0, c.h)
            }
        }));
        let mut out = Block::default();
        // Children's contents move into their boxes.
        let mut boxes: HashMap<&str, (Card, Vec<String>)> = HashMap::new();
        for (child, mut block) in inner {
            let card = cards[&format!("{GROUP}{child}")];
            block.shift(card.x + PAD, card.y + PAD + GROUP_TITLE);
            out.groups.insert(
                child.to_string(),
                Box2 {
                    x: card.x,
                    y: card.y,
                    width: card.w,
                    height: card.h,
                },
            );
            boxes.insert(child, (card, block.nodes.keys().cloned().collect()));
            out.nodes.extend(block.nodes);
            out.groups.extend(block.groups);
            out.routes.extend(block.routes);
        }
        for (id, card) in &cards {
            if let Some(node) = id.strip_prefix(NODE) {
                out.nodes.insert(
                    node.to_string(),
                    Position {
                        x: card.x,
                        y: card.y,
                    },
                );
            }
        }
        for (i, a, b) in &edges {
            let e = &self.input.edges[*i];
            let Some(l) = g.edge(a, b, Some(&i.to_string())) else {
                continue;
            };
            if l.points.len() < 2 {
                continue;
            }
            let raw: Vec<(f64, f64)> = l.points.iter().map(|p| (p.x, p.y)).collect();
            let mut points = if a == b {
                raw
            } else {
                attach(&raw, cards[a], cards[b], horizontal, &bands)
            };
            // An end inside a group: continue from its box to the member, around the others.
            if let Some(group) = a.strip_prefix(GROUP) {
                let (_, members) = &boxes[group];
                if let Some(target) = self.card(&out, &e.from) {
                    let obstacles = self.obstacles(&out, members, &e.from);
                    let mut lead = connect(points[0], target, &obstacles, cards[a]);
                    lead.reverse();
                    lead.extend(points.into_iter().skip(1));
                    points = lead;
                }
            }
            if let Some(group) = b.strip_prefix(GROUP) {
                let (_, members) = &boxes[group];
                if let Some(target) = self.card(&out, &e.to) {
                    let obstacles = self.obstacles(&out, members, &e.to);
                    let end = *points.last().unwrap_or(&(0.0, 0.0));
                    points.pop();
                    points.extend(connect(end, target, &obstacles, cards[b]));
                }
            }
            let label = e.label.and(l.x.zip(l.y)).or_else(|| middle(&l.points));
            out.routes.insert(*i, (points, label));
        }
        // Normalize to the block's own origin.
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for c in cards.values() {
            x0 = x0.min(c.x);
            y0 = y0.min(c.y);
            x1 = x1.max(c.x + c.w);
            y1 = y1.max(c.y + c.h);
        }
        if x0.is_finite() {
            out.shift(-x0, -y0);
            out.width = x1 - x0;
            out.height = y1 - y0;
        }
        Ok(out)
    }
    fn obstacles(&self, block: &Block, members: &[String], except: &str) -> Vec<Card> {
        members
            .iter()
            .filter(|m| m.as_str() != except)
            .filter_map(|m| self.card(block, m))
            .collect()
    }
}

/// A path from `from` (on a group's border) to the nearest side of `target` inside the group,
/// around the other cards: A* on a coarse grid, with turns penalized so it reads as a few
/// straight runs. Falls back to a straight line when no path exists.
fn connect(from: (f64, f64), target: Card, obstacles: &[Card], bounds: Card) -> Vec<(f64, f64)> {
    const STEP: f64 = 10.0;
    const MARGIN: f64 = 8.0;
    let (tx, ty) = target.center();
    // The goal: the middle of the target's side facing the start.
    let (dx, dy) = (from.0 - tx, from.1 - ty);
    let goal_side = if dx.abs() * target.h > dy.abs() * target.w {
        if dx > 0.0 {
            (target.x + target.w, ty)
        } else {
            (target.x, ty)
        }
    } else if dy > 0.0 {
        (tx, target.y + target.h)
    } else {
        (tx, target.y)
    };
    let cols = ((bounds.w / STEP).ceil() as i64 + 1).max(1);
    let rows = ((bounds.h / STEP).ceil() as i64 + 1).max(1);
    let cell = |p: (f64, f64)| {
        (
            (((p.0 - bounds.x) / STEP).round() as i64).clamp(0, cols - 1),
            (((p.1 - bounds.y) / STEP).round() as i64).clamp(0, rows - 1),
        )
    };
    let at = |c: (i64, i64)| (bounds.x + c.0 as f64 * STEP, bounds.y + c.1 as f64 * STEP);
    let blocked = |c: (i64, i64)| {
        let (x, y) = at(c);
        obstacles.iter().chain(std::iter::once(&target)).any(|o| {
            x > o.x - MARGIN && x < o.x + o.w + MARGIN && y > o.y - MARGIN && y < o.y + o.h + MARGIN
        })
    };
    // Just outside the goal side, so the last run enters the card straight.
    let approach = (
        goal_side.0
            + (goal_side.0 - tx).signum() * (MARGIN + STEP) * (goal_side.0 != tx) as i64 as f64,
        goal_side.1
            + (goal_side.1 - ty).signum() * (MARGIN + STEP) * (goal_side.1 != ty) as i64 as f64,
    );
    let (start, goal) = (cell(from), cell(approach));
    let straight = vec![from, goal_side];
    if blocked(goal) {
        return straight;
    }
    // State: cell and direction of arrival (0..4, 4 = none).
    let index = |c: (i64, i64), d: usize| ((c.1 * cols + c.0) as usize) * 5 + d;
    let mut best = vec![f64::INFINITY; (cols * rows) as usize * 5];
    let mut came: HashMap<usize, usize> = HashMap::new();
    let mut open = std::collections::BinaryHeap::new();
    let h = |c: (i64, i64)| ((c.0 - goal.0).abs() + (c.1 - goal.1).abs()) as f64 * STEP;
    best[index(start, 4)] = 0.0;
    open.push(std::cmp::Reverse((ordered(h(start)), start, 4usize)));
    let moves = [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)];
    let mut found = None;
    let mut expanded = 0;
    while let Some(std::cmp::Reverse((_, c, d))) = open.pop() {
        if c == goal {
            found = Some((c, d));
            break;
        }
        expanded += 1;
        if expanded > 200_000 {
            break;
        }
        let g = best[index(c, d)];
        for (k, (mx, my)) in moves.iter().enumerate() {
            let n = (c.0 + mx, c.1 + my);
            if n.0 < 0 || n.1 < 0 || n.0 >= cols || n.1 >= rows || (blocked(n) && n != start) {
                continue;
            }
            let turn = if d != 4 && d != k { 40.0 } else { 0.0 };
            let cost = g + STEP + turn;
            let j = index(n, k);
            if cost < best[j] {
                best[j] = cost;
                came.insert(j, index(c, d));
                open.push(std::cmp::Reverse((ordered(cost + h(n)), n, k)));
            }
        }
    }
    let Some((c, d)) = found else {
        return straight;
    };
    let mut cells = vec![c];
    let mut j = index(c, d);
    while let Some(&prev) = came.get(&j) {
        let cellno = (prev / 5) as i64;
        cells.push((cellno % cols, cellno / cols));
        j = prev;
    }
    cells.reverse();
    // Keep only the corners.
    let mut points = vec![from];
    for w in cells.windows(3) {
        let (a, b, c) = (w[0], w[1], w[2]);
        if (b.0 - a.0, b.1 - a.1) != (c.0 - b.0, c.1 - b.1) {
            points.push(at(b));
        }
    }
    points.push(at(goal));
    points.push(goal_side);
    points
}

/// f64 ordered for the A* heap (costs are finite and non-negative).
fn ordered(v: f64) -> u64 {
    (v * 16.0) as u64
}

fn hierarchical(input: &FlowInput) -> Result<FlowLayout, String> {
    let tree = Tree::new(input);
    let block = tree.block(None)?;
    let mut out = FlowLayout {
        nodes: block.nodes,
        groups: block.groups,
        routes: vec![],
    };
    out.routes = (0..input.edges.len())
        .map(|i| {
            block.routes.get(&i).map(|(points, label)| Route {
                points: points.clone(),
                label: *label,
            })
        })
        .collect();
    shift_to_origin(&mut out);
    Ok(out)
}

/// How many ways a layout is unclean: overlapping cards, cards inside groups they do not
/// belong to, members outside (or under the title of) their group, edges through other cards.
pub fn problems(input: &FlowInput, out: &FlowLayout) -> usize {
    let cards: HashMap<&str, Card> = input
        .nodes
        .iter()
        .filter_map(|n| {
            let p = out.nodes.get(&n.id)?;
            Some((
                n.id.as_str(),
                Card {
                    x: p.x,
                    y: p.y,
                    w: n.width,
                    h: n.height,
                },
            ))
        })
        .collect();
    let overlap = |a: &Card, b: &Card| {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    };
    let mut count = input.nodes.len() - cards.len();
    let ids: Vec<&str> = {
        let mut ids: Vec<&str> = cards.keys().copied().collect();
        ids.sort_unstable();
        ids
    };
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            count += overlap(&cards[a], &cards[b]) as usize;
        }
    }
    let tree = Tree::new(input);
    for g in &input.groups {
        let Some(b) = out.groups.get(&g.id) else {
            continue;
        };
        let area = Card {
            x: b.x,
            y: b.y,
            w: b.width,
            h: b.height,
        };
        for (id, c) in &cards {
            let member = tree.chain(id).contains(&g.id.as_str());
            if member {
                let inside = c.x >= b.x - 0.5
                    && c.y >= b.y + GROUP_TITLE - 0.5
                    && c.x + c.w <= b.x + b.width + 0.5
                    && c.y + c.h <= b.y + b.height + 0.5;
                count += !inside as usize;
            } else {
                count += overlap(&area, c) as usize;
            }
        }
    }
    for (e, route) in input.edges.iter().zip(&out.routes) {
        let Some(route) = route else { continue };
        for (id, c) in &cards {
            if *id == e.from || *id == e.to {
                continue;
            }
            count += route.points.windows(2).any(|w| crosses(c, w[0], w[1])) as usize;
        }
    }
    count
}

/// Whether a segment passes through the inside of a card (shrunk a little).
fn crosses(c: &Card, a: (f64, f64), b: (f64, f64)) -> bool {
    let inset = 4.0;
    let (x0, y0, x1, y1) = (
        c.x + inset,
        c.y + inset,
        c.x + c.w - inset,
        c.y + c.h - inset,
    );
    let steps = (((b.0 - a.0).abs() + (b.1 - a.1).abs()) / 8.0)
        .ceil()
        .max(1.0) as usize;
    (0..=steps).any(|i| {
        let t = i as f64 / steps as f64;
        let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        x > x0 && x < x1 && y > y0 && y < y1
    })
}

#[derive(Clone, Copy, Debug)]
struct Card {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}
impl Card {
    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}
/// Room kept between an edge and the cards of a rank it passes.
const CLEAR: f64 = 12.0;

/// The extent of every rank along the rank axis, keyed by the rank line's position.
struct Bands(Vec<(f64, f64, f64)>);
impl Bands {
    fn new(cards: impl Iterator<Item = (f64, f64)>) -> Self {
        let mut bands: Vec<(f64, f64, f64)> = vec![];
        for (center, size) in cards {
            let (lo, hi) = (center - size / 2.0, center + size / 2.0);
            match bands.iter_mut().find(|b| (b.0 - center).abs() < 1.0) {
                Some(b) => {
                    b.1 = b.1.min(lo);
                    b.2 = b.2.max(hi);
                }
                None => bands.push((center, lo, hi)),
            }
        }
        Bands(bands)
    }
    /// The rank whose line passes through `axis`, as (start, end).
    fn at(&self, axis: f64) -> Option<(f64, f64)> {
        self.0
            .iter()
            .find(|b| (b.0 - axis).abs() < 1.0)
            .map(|b| (b.1, b.2))
    }
}

/// Re-attach a route's ends to the sides of its cards that face the flow, and run it straight
/// through every rank it crosses, keeping dugong's waypoints. Dagre aims ends at card centers
/// and joins waypoints diagonally, which can cut a neighbor that is taller than the rank line
/// suggests; crossing ranks along the rank axis, and turning only between ranks, cannot.
fn attach(
    raw: &[(f64, f64)],
    from: Card,
    to: Card,
    horizontal: bool,
    bands: &Bands,
) -> Vec<(f64, f64)> {
    let inner = &raw[1..raw.len() - 1];
    let (fc, tc) = (from.center(), to.center());
    let along = |p: (f64, f64)| if horizontal { p.0 } else { p.1 };
    let across = |p: (f64, f64)| if horizontal { p.1 } else { p.0 };
    let point = |a: f64, c: f64| if horizontal { (a, c) } else { (c, a) };
    let (a, b) = (along(fc), along(tc));
    let span = if horizontal { from.w } else { from.h } / 2.0;
    if (b - a).abs() < span {
        // Same rank: a flat edge, drawn straight between facing sides across the rank.
        return flat(from, to, horizontal);
    }
    let dir = if b >= a { 1.0 } else { -1.0 };
    let side = |c: Card, out: bool| {
        let (lo, hi) = if horizontal {
            (c.x, c.x + c.w)
        } else {
            (c.y, c.y + c.h)
        };
        if (dir > 0.0) == out {
            hi
        } else {
            lo
        }
    };
    let band_edge = |axis: f64, out: bool, fallback: f64| {
        bands
            .at(axis)
            .map(|(lo, hi)| if (dir > 0.0) == out { hi } else { lo })
            .unwrap_or(fallback)
    };
    let next = inner.first().copied().unwrap_or(tc);
    let prev = inner.last().copied().unwrap_or(fc);
    let clamp = |v: f64, lo: f64, hi: f64| {
        if hi - lo < 24.0 {
            (lo + hi) / 2.0
        } else {
            v.clamp(lo + 10.0, hi - 10.0)
        }
    };
    let (from_lo, from_hi, to_lo, to_hi) = if horizontal {
        (from.y, from.y + from.h, to.y, to.y + to.h)
    } else {
        (from.x, from.x + from.w, to.x, to.x + to.w)
    };
    let s = clamp(across(next), from_lo, from_hi);
    let t = clamp(across(prev), to_lo, to_hi);
    let mut points = vec![point(side(from, true), s)];
    points.push(point(band_edge(a, true, side(from, true)) + dir * CLEAR, s));
    for &p in inner {
        // A waypoint on a rank line is a slot among that rank's cards: pass the whole rank.
        match bands.at(along(p)) {
            Some((lo, hi)) => {
                let (enter, leave) = if dir > 0.0 { (lo, hi) } else { (hi, lo) };
                points.push(point(enter - dir * CLEAR, across(p)));
                points.push(point(leave + dir * CLEAR, across(p)));
            }
            None => points.push(p),
        }
    }
    points.push(point(band_edge(b, false, side(to, false)) - dir * CLEAR, t));
    points.push(point(side(to, false), t));
    points
}

/// Two cards in the same rank: straight across, between the sides that face each other.
fn flat(from: Card, to: Card, horizontal: bool) -> Vec<(f64, f64)> {
    let (fc, tc) = (from.center(), to.center());
    if horizontal {
        // Ranks are columns: same-rank cards sit above one another.
        let (sy, ey) = if tc.1 >= fc.1 {
            (from.y + from.h, to.y)
        } else {
            (from.y, to.y + to.h)
        };
        vec![(fc.0, sy), (tc.0, ey)]
    } else {
        let (sx, ex) = if tc.0 >= fc.0 {
            (from.x + from.w, to.x)
        } else {
            (from.x, to.x + to.w)
        };
        vec![(sx, fc.1), (ex, tc.1)]
    }
}

fn middle(points: &[dugong::Point]) -> Option<(f64, f64)> {
    let p = points.get(points.len() / 2)?;
    Some((p.x, p.y))
}

/// Put the drawing's top-left corner at (0, 0).
fn shift_to_origin(out: &mut FlowLayout) {
    let xs = out
        .nodes
        .values()
        .map(|p| p.x)
        .chain(out.groups.values().map(|b| b.x));
    let ys = out
        .nodes
        .values()
        .map(|p| p.y)
        .chain(out.groups.values().map(|b| b.y));
    let (dx, dy) = (
        xs.fold(f64::INFINITY, f64::min),
        ys.fold(f64::INFINITY, f64::min),
    );
    if !dx.is_finite() || !dy.is_finite() {
        return;
    }
    for p in out.nodes.values_mut() {
        p.x -= dx;
        p.y -= dy;
    }
    for b in out.groups.values_mut() {
        b.x -= dx;
        b.y -= dy;
    }
    for route in out.routes.iter_mut().flatten() {
        for p in &mut route.points {
            p.0 -= dx;
            p.1 -= dy;
        }
        if let Some(l) = &mut route.label {
            l.0 -= dx;
            l.1 -= dy;
        }
    }
}

/// A smooth SVG path through a route's points (Mermaid's `basis` curve), shortened at the end
/// by `gap` so the arrowhead stays clear of the card.
pub fn route_path(points: &[(f64, f64)], gap: f64) -> String {
    let mut pts = points.to_vec();
    if pts.len() < 2 {
        return String::new();
    }
    let n = pts.len();
    let (ax, ay) = pts[n - 2];
    let (bx, by) = pts[n - 1];
    let len = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
    if len > gap * 2.0 {
        pts[n - 1] = (bx - (bx - ax) / len * gap, by - (by - ay) / len * gap);
    }
    let mut d = format!("M{:.1},{:.1}", pts[0].0, pts[0].1);
    if pts.len() == 2 {
        d.push_str(&format!(" L{:.1},{:.1}", pts[1].0, pts[1].1));
        return d;
    }
    // Quadratic segments through midpoints: smooth, and passes near every waypoint.
    for i in 1..pts.len() - 1 {
        let (cx, cy) = pts[i];
        let (nx, ny) = pts[i + 1];
        let (mx, my) = if i == pts.len() - 2 {
            (nx, ny)
        } else {
            ((cx + nx) / 2.0, (cy + ny) / 2.0)
        };
        d.push_str(&format!(" Q{cx:.1},{cy:.1} {mx:.1},{my:.1}"));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_layout::Rect;

    fn sample() -> Diagram {
        serde_json::from_str(include_str!("sample-diagram.json")).unwrap()
    }
    fn rects(d: &Diagram, out: &FlowLayout) -> HashMap<String, Rect> {
        d.nodes
            .iter()
            .map(|n| {
                let p = out.nodes[&n.id];
                (
                    n.id.clone(),
                    Rect {
                        x: p.x,
                        y: p.y,
                        w: CARD_WIDTH,
                        h: card_height(n),
                    },
                )
            })
            .collect()
    }
    fn overlap(a: &Rect, b: &Rect) -> bool {
        a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
    }
    /// Whether the segment crosses the inside of the rectangle (shrunk a little).
    fn crosses(r: &Rect, a: (f64, f64), b: (f64, f64)) -> bool {
        let inset = 4.0;
        let (x0, y0, x1, y1) = (
            r.x + inset,
            r.y + inset,
            r.x + r.w - inset,
            r.y + r.h - inset,
        );
        (0..=40).any(|i| {
            let t = i as f64 / 40.0;
            let (x, y) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            x > x0 && x < x1 && y > y0 && y < y1
        })
    }
    /// A layered "service map": several chains, a cycle, a fan-out and nested groups.
    fn generated(n: usize, seed: u64) -> Diagram {
        let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut next = |m: usize| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as usize) % m.max(1)
        };
        let mut d = Diagram {
            kind: "flowchart".into(),
            direction: Some(if seed.is_multiple_of(2) { "LR" } else { "TD" }.into()),
            ..Diagram::default()
        };
        for i in 0..n {
            let mut node = crate::types::DiagramNode {
                id: format!("n{i}"),
                label: format!("Node {i}"),
                ..Default::default()
            };
            if next(4) == 0 {
                node.annotation.note = Some("Has a description".into());
            }
            if next(6) == 0 {
                node.details = (0..next(12)).map(|k| format!("field{k} int")).collect();
            }
            d.nodes.push(node);
        }
        for i in 1..n {
            d.edges.push(crate::types::DiagramEdge {
                from: format!("n{}", next(i)),
                to: format!("n{i}"),
                label: (next(3) == 0).then(|| "calls".into()),
                ..Default::default()
            });
        }
        for _ in 0..n / 4 {
            let (a, b) = (next(n), next(n));
            d.edges.push(crate::types::DiagramEdge {
                from: format!("n{a}"),
                to: format!("n{b}"),
                ..Default::default()
            });
        }
        let outer: Vec<String> = (0..n / 3).map(|i| format!("n{i}")).collect();
        let inner: Vec<String> = (n / 3..n / 2).map(|i| format!("n{i}")).collect();
        d.groups.push(crate::types::DiagramGroup {
            id: "outer".into(),
            title: "Outer".into(),
            nodes: outer,
            ..Default::default()
        });
        d.groups.push(crate::types::DiagramGroup {
            id: "inner".into(),
            title: "Inner".into(),
            nodes: inner,
            // Nested in two seeds out of three, side by side otherwise.
            parent: (!seed.is_multiple_of(3)).then(|| "outer".into()),
            ..Default::default()
        });
        d
    }

    fn check(d: &Diagram) -> FlowLayout {
        let out = layout(&input(d)).unwrap();
        let boxes = rects(d, &out);
        let ids: Vec<&String> = boxes.keys().collect();
        // No two cards overlap.
        for (i, a) in ids.iter().enumerate() {
            for b in &ids[i + 1..] {
                assert!(!overlap(&boxes[*a], &boxes[*b]), "{a} overlaps {b}");
            }
        }
        // No edge runs through a card that is not one of its ends.
        for (e, route) in d.edges.iter().zip(&out.routes) {
            let Some(route) = route else { continue };
            for (id, r) in &boxes {
                if *id == e.from || *id == e.to {
                    continue;
                }
                for w in route.points.windows(2) {
                    assert!(
                        !crosses(r, w[0], w[1]),
                        "{}->{} crosses {id} {r:?}: {:?} from {:?} to {:?} ({:?})",
                        e.from,
                        e.to,
                        route.points,
                        boxes[&e.from],
                        boxes[&e.to],
                        d.direction
                    );
                }
            }
        }
        // Cards outside a group stay clear of its box, title included.
        for g in &d.groups {
            let Some(b) = out.groups.get(&g.id) else {
                continue;
            };
            let members = group_members(d, &g.id);
            let area = Rect {
                x: b.x,
                y: b.y,
                w: b.width,
                h: b.height,
            };
            for (id, r) in &boxes {
                if !members.contains(id) {
                    assert!(
                        !overlap(&area, r),
                        "{id} {r:?} overlaps group {} {area:?} members {members:?} groups {:?} n={} dir {:?}",
                        g.id,
                        d.groups.iter().map(|g| (&g.id, &g.nodes)).collect::<Vec<_>>(),
                        d.nodes.len(),
                        d.direction
                    );
                }
            }
        }
        // Every member sits inside its group's box, below the room for its title.
        for g in &d.groups {
            let Some(b) = out.groups.get(&g.id) else {
                continue;
            };
            for m in group_members(d, &g.id) {
                let r = &boxes[&m];
                assert!(
                    r.x >= b.x - 0.5
                        && r.y >= b.y + GROUP_TITLE - 0.5
                        && r.x + r.w <= b.x + b.width + 0.5
                        && r.y + r.h <= b.y + b.height + 0.5,
                    "{m} outside {}",
                    g.id
                );
            }
        }
        out
    }

    #[test]
    fn the_sample_flows_left_to_right_inside_its_group() {
        let d = sample();
        let out = check(&d);
        let x = |id: &str| out.nodes[id].x;
        assert!(x("web") < x("api") && x("api") < x("db"), "{:?}", out.nodes);
        assert!(out.groups.contains_key("backend"));
        assert!(out.routes.iter().all(Option::is_some));
        // Same input, same picture.
        assert_eq!(layout(&input(&d)).unwrap(), out);
    }

    #[test]
    fn direction_follows_the_file() {
        let mut d = sample();
        d.direction = Some("TD".into());
        let out = check(&d);
        assert!(out.nodes["web"].y < out.nodes["api"].y);
        d.direction = Some("RL".into());
        let out = check(&d);
        assert!(out.nodes["web"].x > out.nodes["api"].x);
    }

    #[test]
    fn tall_cards_get_room() {
        let mut d = sample();
        d.nodes[2].details = (0..20).map(|i| format!("col{i} text")).collect();
        check(&d);
    }

    #[test]
    fn generated_diagrams_stay_clean() {
        let seeds: u64 = option_env!("SCHEMATLAS_LAYOUT_SEEDS")
            .and_then(|s| s.parse().ok())
            .unwrap_or(12);
        for seed in 0..seeds {
            let n = 6 + (seed as usize * 7) % 40;
            check(&generated(n, seed));
        }
    }

    #[test]
    fn large_diagrams_lay_out_quickly() {
        let d = generated(300, 7);
        let start = std::time::Instant::now();
        let out = layout(&input(&d)).unwrap();
        assert_eq!(out.nodes.len(), 300);
        // Generous for debug builds; release in the worker is far faster.
        assert!(
            start.elapsed().as_secs_f64() < 20.0,
            "{:?}",
            start.elapsed()
        );
    }

    #[test]
    fn routes_become_smooth_paths() {
        assert_eq!(
            route_path(&[(0.0, 0.0), (100.0, 0.0)], 6.0),
            "M0.0,0.0 L94.0,0.0"
        );
        let d = route_path(&[(0.0, 0.0), (50.0, 0.0), (50.0, 50.0), (100.0, 50.0)], 0.0);
        assert!(d.starts_with("M0.0,0.0 Q50.0,0.0"), "{d}");
        assert!(d.ends_with("100.0,50.0"), "{d}");
    }
}
