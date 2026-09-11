//! Card geometry, filtering, group overlays and viewport bounds. Ported from the Svelte services.
use crate::types::{CanvasGroup, Entity, Graph, Position};
use std::collections::{HashMap, HashSet};

pub const NODE_WIDTH: f64 = 284.0;
/// Columns shown on a card; the rest collapse into a "more" row.
pub const MAX_FIELDS: usize = 9;
pub const DEFAULT_GROUP_COLOR: &str = "#879b91";

pub fn node_height(field_count: usize) -> f64 {
    106.0 + field_count.min(MAX_FIELDS) as f64 * 29.0 + if field_count > MAX_FIELDS { 29.0 } else { 0.0 }
}
pub fn entity_height(entity: &Entity) -> f64 {
    node_height(entity.fields.len())
}

pub fn matching_ids(graph: &Graph, query: &str) -> HashSet<String> {
    let q = query.trim().to_lowercase();
    graph
        .entities
        .iter()
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.namespace.to_lowercase().contains(&q)
                || e.method.as_deref().is_some_and(|m| m.to_lowercase().contains(&q))
                || e.fields.iter().any(|f| f.name.to_lowercase().contains(&q))
        })
        .map(|e| e.id.clone())
        .collect()
}

pub fn schema_namespaces(graph: &Graph) -> Vec<(String, usize)> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for e in &graph.entities {
        *counts.entry(e.namespace.as_str()).or_default() += 1;
    }
    let mut list: Vec<(String, usize)> = counts.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    list.sort_by(|a, b| a.0.cmp(&b.0));
    list
}

pub fn filter_graph(graph: &Graph, namespaces: &[String], related: bool) -> Graph {
    if namespaces.is_empty() {
        return graph.clone();
    }
    let selected: HashSet<&str> = graph
        .entities
        .iter()
        .filter(|e| namespaces.contains(&e.namespace))
        .map(|e| e.id.as_str())
        .collect();
    let mut visible = selected.clone();
    if related {
        for r in &graph.relations {
            if selected.contains(r.source.as_str()) || selected.contains(r.target.as_str()) {
                visible.insert(&r.source);
                visible.insert(&r.target);
            }
        }
    }
    Graph {
        entities: graph.entities.iter().filter(|e| visible.contains(e.id.as_str())).cloned().collect(),
        relations: graph
            .relations
            .iter()
            .filter(|r| visible.contains(r.source.as_str()) && visible.contains(r.target.as_str()))
            .cloned()
            .collect(),
        warnings: graph.warnings.clone(),
    }
}

pub fn valid_color(color: Option<&str>) -> String {
    color
        .filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|ch| ch.is_ascii_hexdigit()))
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_GROUP_COLOR.to_string())
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupBox {
    pub group_id: String,
    pub name: String,
    pub color: String,
    pub count: usize,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Overlays are visual bounds around member cards; nodes keep absolute coordinates.
pub fn group_bounds(groups: &[CanvasGroup], graph: &Graph, positions: &HashMap<String, Position>) -> Vec<GroupBox> {
    let entities: HashMap<&str, &Entity> = graph.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    groups
        .iter()
        .filter_map(|group| {
            let mut seen = HashSet::new();
            let members: Vec<(&Entity, Position)> = group
                .node_ids
                .iter()
                .filter(|id| seen.insert(id.as_str()))
                .filter_map(|id| Some((*entities.get(id.as_str())?, *positions.get(id)?)))
                .collect();
            if members.is_empty() {
                return None;
            }
            let x = members.iter().map(|(_, p)| p.x).fold(f64::INFINITY, f64::min) - 28.0;
            let y = members.iter().map(|(_, p)| p.y).fold(f64::INFINITY, f64::min) - 48.0;
            let right = members.iter().map(|(_, p)| p.x + NODE_WIDTH).fold(f64::NEG_INFINITY, f64::max) + 28.0;
            let bottom = members
                .iter()
                .map(|(e, p)| p.y + entity_height(e))
                .fold(f64::NEG_INFINITY, f64::max)
                + 28.0;
            Some(GroupBox {
                group_id: group.id.clone(),
                name: group.name.clone(),
                color: valid_color(group.color.as_deref()),
                count: members.len(),
                x,
                y,
                width: right - x,
                height: bottom - y,
            })
        })
        .collect()
}

/// Translate from a drag-start snapshot, including hidden members, without drift.
pub fn translate_group(
    positions: &HashMap<String, Position>,
    member_ids: &[String],
    delta: Position,
) -> HashMap<String, Position> {
    let mut result = positions.clone();
    let members: HashSet<&String> = member_ids.iter().collect();
    for id in members {
        if let Some(p) = positions.get(id) {
            result.insert(id.clone(), Position { x: p.x + delta.x, y: p.y + delta.y });
        }
    }
    result
}

/// Saved layouts bypass the solver. Unpositioned graphs reuse one fallback until topology changes.
pub struct PositionResolver {
    topology: String,
    fallback: HashMap<String, Position>,
    solve: Box<dyn Fn(&Graph) -> HashMap<String, Position>>,
}

impl PositionResolver {
    pub fn new(solve: impl Fn(&Graph) -> HashMap<String, Position> + 'static) -> Self {
        Self { topology: String::new(), fallback: HashMap::new(), solve: Box::new(solve) }
    }
    pub fn resolve(&mut self, graph: &Graph, saved: &HashMap<String, Position>) -> HashMap<String, Position> {
        let valid = |id: &str| saved.get(id).is_some_and(|p| p.x.is_finite() && p.y.is_finite());
        if graph.entities.iter().all(|e| valid(&e.id)) {
            return saved.clone();
        }
        let mut key = String::new();
        for e in &graph.entities {
            key.push_str(&e.id);
            key.push(':');
            key.push_str(&e.fields.len().to_string());
            key.push('|');
        }
        for r in &graph.relations {
            key.push_str(&format!("{}>{}>{}|", r.id, r.source, r.target));
        }
        if key != self.topology {
            self.fallback = (self.solve)(graph);
            self.topology = key;
        }
        graph
            .entities
            .iter()
            .map(|e| {
                let position = if valid(&e.id) {
                    saved[&e.id]
                } else {
                    self.fallback.get(&e.id).copied().unwrap_or_default()
                };
                (e.id.clone(), position)
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Bounds use model positions, including nodes that have never been rendered.
pub fn graph_bounds(graph: &Graph, positions: &HashMap<String, Position>, ids: Option<&HashSet<String>>) -> Option<Bounds> {
    let entities: Vec<(&Entity, Position)> = graph
        .entities
        .iter()
        .filter(|e| ids.is_none_or(|ids| ids.contains(&e.id)))
        .filter_map(|e| {
            let p = positions.get(&e.id)?;
            (p.x.is_finite() && p.y.is_finite()).then_some((e, *p))
        })
        .collect();
    if entities.is_empty() {
        return None;
    }
    let x = entities.iter().map(|(_, p)| p.x).fold(f64::INFINITY, f64::min);
    let y = entities.iter().map(|(_, p)| p.y).fold(f64::INFINITY, f64::min);
    let right = entities.iter().map(|(_, p)| p.x + NODE_WIDTH).fold(f64::NEG_INFINITY, f64::max);
    let bottom = entities
        .iter()
        .map(|(e, p)| p.y + entity_height(e))
        .fold(f64::NEG_INFINITY, f64::max);
    Some(Bounds { x, y, width: right - x, height: bottom - y })
}

/// Immediate fallback while a relationship-aware arrangement is computed.
pub fn grid_positions(graph: &Graph) -> HashMap<String, Position> {
    let columns = ((graph.entities.len() as f64).sqrt().ceil() as usize).max(1);
    let height = graph
        .entities
        .iter()
        .map(entity_height)
        .fold(160.0_f64, f64::max)
        + 60.0;
    graph
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| {
            (
                e.id.clone(),
                Position {
                    x: 40.0 + (i % columns) as f64 * (NODE_WIDTH + 70.0),
                    y: 60.0 + (i / columns) as f64 * height,
                },
            )
        })
        .collect()
}

/// Whole-graph layered layout with page margins, used by tests and as a simple fallback.
pub fn layout_graph(graph: &Graph) -> HashMap<String, Position> {
    if graph.entities.is_empty() {
        return HashMap::new();
    }
    crate::smart_layout::layered(graph, crate::smart_layout::Direction::LeftRight)
        .positions
        .into_iter()
        .map(|(id, p)| (id, Position { x: p.x + 40.0, y: p.y + 40.0 }))
        .collect()
}

#[cfg(test)]
pub(crate) fn sample_graph() -> Graph {
    let project: crate::types::Project = serde_json::from_str(include_str!("sample.json")).unwrap();
    project.sources[0].graph.clone()
}

#[cfg(test)]
pub(crate) fn assert_no_overlap(graph: &Graph, positions: &HashMap<String, Position>) {
    for a in &graph.entities {
        let p = positions[&a.id];
        assert!(p.x.is_finite() && p.y.is_finite());
        for b in &graph.entities {
            if a.id == b.id {
                continue;
            }
            let q = positions[&b.id];
            assert!(
                p.x + NODE_WIDTH <= q.x
                    || q.x + NODE_WIDTH <= p.x
                    || p.y + entity_height(a) <= q.y
                    || q.y + entity_height(b) <= p.y,
                "{} overlaps {}",
                a.id,
                b.id
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn positions_every_node_finitely_with_no_overlapping_bounds() {
        let graph = sample_graph();
        let positions = layout_graph(&graph);
        assert_eq!(positions.len(), graph.entities.len());
        assert_no_overlap(&graph, &positions);
    }
    #[test]
    fn finds_columns_case_insensitively_and_handles_empty_graphs() {
        let graph = sample_graph();
        assert_eq!(matching_ids(&graph, "CUSTOMER_ID").len(), 2);
        assert_eq!(matching_ids(&graph, "nonsense").len(), 0);
        assert!(layout_graph(&Graph::default()).is_empty());
    }
    fn multi() -> Graph {
        let graph = sample_graph();
        Graph {
            entities: ["sales", "billing", "audit"]
                .iter()
                .map(|ns| Entity {
                    id: format!("{ns}:users"),
                    namespace: (*ns).into(),
                    name: "users".into(),
                    ..graph.entities[0].clone()
                })
                .collect(),
            relations: vec![crate::types::Relation {
                id: "cross-schema".into(),
                source: "sales:users".into(),
                target: "billing:users".into(),
                ..graph.relations[0].clone()
            }],
            warnings: vec![],
        }
    }
    #[test]
    fn keeps_identical_table_names_distinct_and_counts_every_schema() {
        let multi = multi();
        assert_eq!(
            schema_namespaces(&multi),
            vec![("audit".to_string(), 1), ("billing".to_string(), 1), ("sales".to_string(), 1)]
        );
        let sales = filter_graph(&multi, &["sales".to_string()], false);
        assert_eq!(sales.entities.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), vec!["sales:users"]);
        assert!(sales.relations.is_empty());
        assert_eq!(filter_graph(&multi, &[], false), multi);
    }
    #[test]
    fn includes_directly_linked_schemas_only_when_requested() {
        let multi = multi();
        let filtered = filter_graph(&multi, &["sales".to_string()], true);
        assert_eq!(
            filtered.entities.iter().map(|e| e.namespace.as_str()).collect::<Vec<_>>(),
            vec!["sales", "billing"]
        );
        assert_eq!(filtered.relations.len(), 1);
        assert_eq!(filter_graph(&multi, &["sales".to_string(), "audit".to_string()], false).entities.len(), 2);
    }
    #[test]
    fn encloses_member_nodes_and_follows_moves() {
        let graph = sample_graph();
        let members: Vec<&Entity> = graph.entities.iter().take(2).collect();
        let mut positions = HashMap::from([
            (members[0].id.clone(), Position { x: 100.0, y: 200.0 }),
            (members[1].id.clone(), Position { x: 500.0, y: 200.0 }),
        ]);
        let groups = vec![CanvasGroup {
            id: "domain".into(),
            name: "Customer domain".into(),
            node_ids: members.iter().map(|e| e.id.clone()).collect(),
            color: None,
        }];
        let initial = group_bounds(&groups, &graph, &positions)[0].clone();
        assert_eq!(initial.x, 72.0);
        assert_eq!(initial.y, 152.0);
        assert_eq!(initial.width, 740.0);
        assert_eq!(initial.count, 2);
        positions.insert(members[1].id.clone(), Position { x: 900.0, y: 500.0 });
        let moved = group_bounds(&groups, &graph, &positions)[0].clone();
        assert_eq!(moved.width, 1140.0);
        assert!(moved.height > initial.height);
        let empty = Graph { entities: vec![], ..graph.clone() };
        assert!(group_bounds(&groups, &empty, &positions).is_empty());
    }
    #[test]
    fn moves_every_member_from_the_snapshot() {
        let positions = HashMap::from([
            ("visible".to_string(), Position { x: 10.0, y: 20.0 }),
            ("hidden".to_string(), Position { x: 300.0, y: 80.0 }),
            ("other".to_string(), Position { x: 900.0, y: 400.0 }),
        ]);
        let members: Vec<String> = ["visible", "hidden", "hidden", "missing"].iter().map(|s| s.to_string()).collect();
        let moved = translate_group(&positions, &members, Position { x: 80.0, y: -50.0 });
        assert_eq!(moved["visible"], Position { x: 90.0, y: -30.0 });
        assert_eq!(moved["hidden"], Position { x: 380.0, y: 30.0 });
        assert_eq!(moved["other"], positions["other"]);
        assert_eq!(moved.len(), 3);
        assert_eq!(translate_group(&positions, &members, Position { x: 100.0, y: 0.0 })["hidden"], Position { x: 400.0, y: 80.0 });
    }
    #[test]
    fn does_not_solve_a_fully_positioned_graph() {
        let calls = Rc::new(Cell::new(0));
        let counter = calls.clone();
        let mut resolve = PositionResolver::new(move |g| {
            counter.set(counter.get() + 1);
            layout_graph(g)
        });
        let graph = sample_graph();
        let saved = layout_graph(&graph);
        assert_eq!(resolve.resolve(&graph, &saved), saved);
        assert_eq!(resolve.resolve(&graph.clone(), &saved), saved);
        assert_eq!(calls.get(), 0);
    }
    #[test]
    fn reuses_the_fallback_but_recalculates_changed_topology() {
        let calls = Rc::new(Cell::new(0));
        let counter = calls.clone();
        let mut resolve = PositionResolver::new(move |g| {
            counter.set(counter.get() + 1);
            layout_graph(g)
        });
        let graph = sample_graph();
        resolve.resolve(&graph, &HashMap::new());
        resolve.resolve(&graph.clone(), &HashMap::new());
        assert_eq!(calls.get(), 1);
        let mut changed = graph.clone();
        let extra = crate::types::Field { name: "extra".into(), ..changed.entities[0].fields[0].clone() };
        changed.entities[0].fields.push(extra);
        let id = changed.entities[0].id.clone();
        let position = Position { x: 45.0, y: 90.0 };
        let saved = HashMap::from([(id.clone(), position)]);
        assert_eq!(resolve.resolve(&changed, &saved)[&id], position);
        assert_eq!(calls.get(), 2);
    }
    #[test]
    fn bounds_include_distant_nodes_and_respect_focus() {
        let graph = sample_graph();
        let distant = Graph { entities: graph.entities[..2].to_vec(), relations: vec![], warnings: vec![] };
        let (a, b) = (&distant.entities[0], &distant.entities[1]);
        let positions = HashMap::from([
            (a.id.clone(), Position { x: -9000.0, y: -6000.0 }),
            (b.id.clone(), Position { x: 24000.0, y: 19000.0 }),
        ]);
        assert_eq!(
            graph_bounds(&distant, &positions, None),
            Some(Bounds { x: -9000.0, y: -6000.0, width: 33000.0 + NODE_WIDTH, height: 25000.0 + entity_height(b) })
        );
        let focus = HashSet::from([b.id.clone()]);
        assert_eq!(
            graph_bounds(&distant, &positions, Some(&focus)),
            Some(Bounds { x: 24000.0, y: 19000.0, width: NODE_WIDTH, height: entity_height(b) })
        );
        assert_eq!(graph_bounds(&distant, &positions, Some(&HashSet::new())), None);
    }
}
