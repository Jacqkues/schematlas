//! Relationship-aware arrangement: layered layouts per connected component, shelf packing per domain.
//! Replaces the Dagre-based worker with a compact Sugiyama-style layout in Rust.
use crate::layout::{entity_height, graph_bounds, NODE_WIDTH};
use crate::types::{CanvasGroup, Graph, Position};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    LeftRight,
    TopBottom,
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    pub positions: HashMap<String, Position>,
    pub width: f64,
    pub height: f64,
}

/// Stable shelf packing keeps disconnected components and domains compact and separate.
fn pack(blocks: &[Block], gap: f64) -> Block {
    let mut sorted: Vec<&Block> = blocks.iter().collect();
    sorted.sort_by(|a, b| {
        b.height
            .partial_cmp(&a.height)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.width
                    .partial_cmp(&a.width)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    let area: f64 = blocks
        .iter()
        .map(|b| (b.width + gap) * (b.height + gap))
        .sum();
    let target = blocks
        .iter()
        .map(|b| b.width)
        .fold(1.0_f64, f64::max)
        .max((area * 1.5).sqrt());
    let mut positions = HashMap::new();
    let (mut x, mut y, mut row, mut width) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for block in sorted {
        if x > 0.0 && x + block.width > target {
            y += row + gap;
            x = 0.0;
            row = 0.0;
        }
        for (id, p) in &block.positions {
            positions.insert(
                id.clone(),
                Position {
                    x: x + p.x,
                    y: y + p.y,
                },
            );
        }
        width = width.max(x + block.width);
        row = row.max(block.height);
        x += block.width + gap;
    }
    Block {
        positions,
        width,
        height: y + row,
    }
}

/// Connected components in breadth-first order, so neighbors stay adjacent.
fn connected(graph: &Graph) -> Vec<Graph> {
    let index: HashMap<&str, usize> = graph
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); graph.entities.len()];
    for r in &graph.relations {
        if let (Some(&u), Some(&v)) = (index.get(r.source.as_str()), index.get(r.target.as_str())) {
            adjacency[u].push(v);
            adjacency[v].push(u);
        }
    }
    let mut seen = vec![false; graph.entities.len()];
    let mut result = Vec::new();
    for start in 0..graph.entities.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut order = vec![start];
        let mut queue = VecDeque::from([start]);
        while let Some(u) = queue.pop_front() {
            for &v in &adjacency[u] {
                if !seen[v] {
                    seen[v] = true;
                    order.push(v);
                    queue.push_back(v);
                }
            }
        }
        let ids: HashSet<&str> = order
            .iter()
            .map(|&i| graph.entities[i].id.as_str())
            .collect();
        result.push(Graph {
            entities: order.iter().map(|&i| graph.entities[i].clone()).collect(),
            relations: graph
                .relations
                .iter()
                .filter(|r| ids.contains(r.source.as_str()) && ids.contains(r.target.as_str()))
                .cloned()
                .collect(),
            warnings: vec![],
        });
    }
    result
}

/// Layered layout: cycle breaking, longest-path ranking, barycenter ordering, stacked coordinates.
pub fn layered(graph: &Graph, direction: Direction) -> Block {
    let n = graph.entities.len();
    if n == 0 {
        return Block::default();
    }
    let index: HashMap<&str, usize> = graph
        .entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut edges = HashSet::new();
    for r in &graph.relations {
        if let (Some(&u), Some(&v)) = (index.get(r.source.as_str()), index.get(r.target.as_str())) {
            if u != v && edges.insert((u, v)) {
                out[u].push(v);
            }
        }
    }
    // Break cycles: edges closing a cycle during DFS are reversed for ranking purposes.
    let mut state = vec![0u8; n];
    let mut reversed: HashSet<(usize, usize)> = HashSet::new();
    for start in 0..n {
        if state[start] != 0 {
            continue;
        }
        state[start] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(top) = stack.last_mut() {
            let u = top.0;
            if top.1 < out[u].len() {
                let v = out[u][top.1];
                top.1 += 1;
                match state[v] {
                    0 => {
                        state[v] = 1;
                        stack.push((v, 0));
                    }
                    1 => {
                        reversed.insert((u, v));
                    }
                    _ => {}
                }
            } else {
                state[u] = 2;
                stack.pop();
            }
        }
    }
    let mut dag_out: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut dag_in: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &(u, v) in &edges {
        let (a, b) = if reversed.contains(&(u, v)) {
            (v, u)
        } else {
            (u, v)
        };
        dag_out[a].push(b);
        dag_in[b].push(a);
    }
    for list in dag_out.iter_mut().chain(dag_in.iter_mut()) {
        list.sort_unstable();
    }
    // Longest-path ranking over the DAG.
    let mut indegree: Vec<usize> = dag_in.iter().map(Vec::len).collect();
    let mut rank = vec![0usize; n];
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    while let Some(u) = queue.pop_front() {
        for &v in &dag_out[u] {
            rank[v] = rank[v].max(rank[u] + 1);
            indegree[v] -= 1;
            if indegree[v] == 0 {
                queue.push_back(v);
            }
        }
    }
    let layer_count = rank.iter().copied().max().unwrap_or(0) + 1;
    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); layer_count];
    for i in 0..n {
        layers[rank[i]].push(i);
    }
    // Barycenter sweeps reduce crossings; ties keep the previous order.
    let mut position = vec![0.0_f64; n];
    let assign = |layers: &Vec<Vec<usize>>, position: &mut Vec<f64>| {
        for layer in layers {
            for (i, &v) in layer.iter().enumerate() {
                position[v] = i as f64;
            }
        }
    };
    assign(&layers, &mut position);
    let barycenter = |v: usize, neighbors: &Vec<usize>, position: &Vec<f64>| -> f64 {
        if neighbors.is_empty() {
            position[v]
        } else {
            neighbors.iter().map(|&u| position[u]).sum::<f64>() / neighbors.len() as f64
        }
    };
    for _ in 0..4 {
        for l in 1..layer_count {
            let mut keyed: Vec<(f64, usize)> = layers[l]
                .iter()
                .map(|&v| (barycenter(v, &dag_in[v], &position), v))
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            layers[l] = keyed.into_iter().map(|(_, v)| v).collect();
            assign(&layers, &mut position);
        }
        for l in (0..layer_count.saturating_sub(1)).rev() {
            let mut keyed: Vec<(f64, usize)> = layers[l]
                .iter()
                .map(|&v| (barycenter(v, &dag_out[v], &position), v))
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            layers[l] = keyed.into_iter().map(|(_, v)| v).collect();
            assign(&layers, &mut position);
        }
    }
    // Coordinates: ranks advance along the main axis, members stack along the other, centered.
    let (nodesep, ranksep) = (56.0, 110.0);
    let heights: Vec<f64> = graph.entities.iter().map(entity_height).collect();
    let mut positions = HashMap::new();
    match direction {
        Direction::LeftRight => {
            let stack_heights: Vec<f64> = layers
                .iter()
                .map(|layer| {
                    layer.iter().map(|&v| heights[v]).sum::<f64>()
                        + nodesep * layer.len().saturating_sub(1) as f64
                })
                .collect();
            let tallest = stack_heights.iter().copied().fold(0.0, f64::max);
            for (l, layer) in layers.iter().enumerate() {
                let mut y = (tallest - stack_heights[l]) / 2.0;
                for &v in layer {
                    positions.insert(
                        graph.entities[v].id.clone(),
                        Position {
                            x: l as f64 * (NODE_WIDTH + ranksep),
                            y,
                        },
                    );
                    y += heights[v] + nodesep;
                }
            }
        }
        Direction::TopBottom => {
            let stack_widths: Vec<f64> = layers
                .iter()
                .map(|layer| {
                    NODE_WIDTH * layer.len() as f64 + nodesep * layer.len().saturating_sub(1) as f64
                })
                .collect();
            let widest = stack_widths.iter().copied().fold(0.0, f64::max);
            let mut y = 0.0;
            for (l, layer) in layers.iter().enumerate() {
                let mut x = (widest - stack_widths[l]) / 2.0;
                let row_height = layer.iter().map(|&v| heights[v]).fold(0.0, f64::max);
                for &v in layer {
                    positions.insert(graph.entities[v].id.clone(), Position { x, y });
                    x += NODE_WIDTH + nodesep;
                }
                y += row_height + ranksep;
            }
        }
    }
    normalize(graph, positions)
}

fn normalize(graph: &Graph, positions: HashMap<String, Position>) -> Block {
    let Some(bounds) = graph_bounds(graph, &positions, None) else {
        return Block::default();
    };
    Block {
        positions: positions
            .into_iter()
            .map(|(id, p)| {
                (
                    id,
                    Position {
                        x: p.x - bounds.x,
                        y: p.y - bounds.y,
                    },
                )
            })
            .collect(),
        width: bounds.width,
        height: bounds.height,
    }
}

/// Dense or long-chain domains use a neighbor-ordered grid when ranks waste substantial space.
fn compact_grid(graph: &Graph) -> Block {
    let mut degree: HashMap<&str, usize> =
        graph.entities.iter().map(|e| (e.id.as_str(), 0)).collect();
    for r in &graph.relations {
        *degree.entry(r.source.as_str()).or_default() += 1;
        *degree.entry(r.target.as_str()).or_default() += 1;
    }
    let mut ordered = graph.clone();
    ordered.entities.sort_by(|a, b| {
        degree[b.id.as_str()]
            .cmp(&degree[a.id.as_str()])
            .then(a.id.cmp(&b.id))
    });
    let walk: Vec<crate::types::Entity> = connected(&ordered)
        .into_iter()
        .flat_map(|part| part.entities)
        .collect();
    let limit = ((walk.len() as f64).sqrt().ceil() as usize) * 2;
    let mut best: Option<(f64, Block)> = None;
    for columns in 1..=limit.max(1) {
        let mut positions = HashMap::new();
        let mut y = 0.0;
        let mut width = 0.0_f64;
        for row in walk.chunks(columns) {
            for (i, e) in row.iter().enumerate() {
                positions.insert(
                    e.id.clone(),
                    Position {
                        x: i as f64 * (NODE_WIDTH + 100.0),
                        y,
                    },
                );
            }
            width = width.max(row.len() as f64 * (NODE_WIDTH + 100.0) - 100.0);
            y += row.iter().map(entity_height).fold(0.0, f64::max) + 80.0;
        }
        let block = Block {
            positions,
            width,
            height: y - 80.0,
        };
        let cost = block.width
            * block.height
            * (1.0 + (block.width / block.height / 1.4).ln().abs() * 0.5);
        if best.as_ref().is_none_or(|(c, _)| cost < *c) {
            best = Some((cost, block));
        }
    }
    best.map(|(_, b)| b).unwrap_or_default()
}

fn score(block: &Block) -> f64 {
    block.width * block.height * (1.0 + (block.width / block.height / 1.5).ln().abs() * 0.35)
}

pub fn smart_layout(graph: &Graph, groups: &[CanvasGroup]) -> HashMap<String, Position> {
    if graph.entities.is_empty() {
        return HashMap::new();
    }
    if crate::openapi_layout::is_openapi(graph) {
        return crate::openapi_layout::arrange(graph, groups);
    }
    // Overlapping groups share a region, so no overlay spans unrelated domains.
    let mut owner: HashMap<String, String> = HashMap::new();
    for group in groups {
        let overlaps: HashSet<String> = group
            .node_ids
            .iter()
            .filter_map(|id| owner.get(id).cloned())
            .collect();
        let region = overlaps
            .iter()
            .min()
            .cloned()
            .unwrap_or_else(|| format!("group:{}", group.id));
        for value in owner.values_mut() {
            if overlaps.contains(value) {
                *value = region.clone();
            }
        }
        for id in &group.node_ids {
            owner.insert(id.clone(), region.clone());
        }
    }
    let mut region_order: Vec<String> = Vec::new();
    let mut regions: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, e) in graph.entities.iter().enumerate() {
        let key = owner
            .get(&e.id)
            .cloned()
            .unwrap_or_else(|| format!("schema:{}", e.namespace));
        regions
            .entry(key.clone())
            .or_insert_with(|| {
                region_order.push(key);
                Vec::new()
            })
            .push(i);
    }
    let blocks: Vec<Block> = region_order
        .iter()
        .map(|key| {
            let ids: HashSet<&str> = regions[key]
                .iter()
                .map(|&i| graph.entities[i].id.as_str())
                .collect();
            let domain = Graph {
                entities: regions[key]
                    .iter()
                    .map(|&i| graph.entities[i].clone())
                    .collect(),
                relations: graph
                    .relations
                    .iter()
                    .filter(|r| ids.contains(r.source.as_str()) && ids.contains(r.target.as_str()))
                    .cloned()
                    .collect(),
                warnings: vec![],
            };
            let components: Vec<Block> = connected(&domain)
                .iter()
                .map(|component| {
                    let lr = layered(component, Direction::LeftRight);
                    let tb = layered(component, Direction::TopBottom);
                    let ranked = if score(&lr) <= score(&tb) { lr } else { tb };
                    let compact = compact_grid(component);
                    if score(&ranked) > score(&compact) * 1.35 {
                        compact
                    } else {
                        ranked
                    }
                })
                .collect();
            pack(&components, 80.0)
        })
        .collect();
    pack(&blocks, 160.0)
        .positions
        .into_iter()
        .map(|(id, p)| {
            (
                id,
                Position {
                    x: p.x + 60.0,
                    y: p.y + 80.0,
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{assert_no_overlap, group_bounds};
    use crate::types::{Entity, Field, Relation};

    #[test]
    fn packs_a_large_cyclic_disconnected_grouped_schema_without_overlaps() {
        let mut graph = Graph {
            entities: (0..164)
                .map(|i| Entity {
                    id: format!("n{i}"),
                    name: format!("table{i}"),
                    namespace: if i < 140 { "public" } else { "auth" }.into(),
                    kind: "table".into(),
                    fields: (0..i % 12)
                        .map(|j| Field {
                            name: format!("f{j}"),
                            data_type: "int".into(),
                            required: true,
                            ..Field::default()
                        })
                        .collect(),
                    ..Entity::default()
                })
                .collect(),
            relations: vec![],
            warnings: vec![],
        };
        for i in 0..150 {
            graph.relations.push(Relation {
                id: format!("r{i}"),
                source: format!("n{i}"),
                target: format!("n{}", (i / 10) * 10 + (i + 1) % 10),
                label: "fk".into(),
                ..Relation::default()
            });
        }
        let groups: Vec<CanvasGroup> = (0..16)
            .map(|i| CanvasGroup {
                id: format!("g{i}"),
                name: format!("Domain {i}"),
                node_ids: graph.entities[i * 10..i * 10 + 10]
                    .iter()
                    .map(|e| e.id.clone())
                    .collect(),
                color: None,
            })
            .collect();
        let positions = smart_layout(&graph, &groups);
        assert_eq!(positions.len(), 164);
        assert_no_overlap(&graph, &positions);
        let bounds = group_bounds(&groups, &graph, &positions);
        for i in 0..bounds.len() {
            for j in i + 1..bounds.len() {
                let (a, b) = (&bounds[i], &bounds[j]);
                assert!(
                    a.x + a.width <= b.x
                        || b.x + b.width <= a.x
                        || a.y + a.height <= b.y
                        || b.y + b.height <= a.y,
                    "group {} overlaps {}",
                    a.name,
                    b.name
                );
            }
        }
        let area = graph_bounds(&graph, &positions, None).unwrap();
        let ratio = area.width / area.height;
        assert!(ratio > 0.5 && ratio < 3.0, "aspect ratio {ratio}");
        assert_eq!(smart_layout(&graph, &groups), positions);
    }
    #[test]
    fn handles_overlapping_groups_singletons_and_empty_graphs() {
        assert!(smart_layout(&Graph::default(), &[]).is_empty());
        let graph = Graph {
            entities: ["a", "b", "c"]
                .iter()
                .map(|id| Entity {
                    id: (*id).into(),
                    name: (*id).into(),
                    namespace: "public".into(),
                    kind: "table".into(),
                    ..Entity::default()
                })
                .collect(),
            relations: vec![],
            warnings: vec![],
        };
        let p = smart_layout(
            &graph,
            &[
                CanvasGroup {
                    id: "g1".into(),
                    name: "One".into(),
                    node_ids: vec!["a".into(), "b".into()],
                    color: None,
                },
                CanvasGroup {
                    id: "g2".into(),
                    name: "Two".into(),
                    node_ids: vec!["b".into(), "c".into(), "missing".into()],
                    color: None,
                },
            ],
        );
        let mut ids: Vec<&String> = p.keys().collect();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }
    #[test]
    fn layered_keeps_cycles_and_isolated_nodes_finite() {
        let graph = Graph {
            entities: ["a", "b", "c", "d"]
                .iter()
                .map(|id| Entity {
                    id: (*id).into(),
                    name: (*id).into(),
                    kind: "table".into(),
                    ..Entity::default()
                })
                .collect(),
            relations: [("a", "b"), ("b", "c"), ("c", "a")]
                .iter()
                .map(|(s, t)| Relation {
                    id: format!("{s}{t}"),
                    source: (*s).into(),
                    target: (*t).into(),
                    ..Relation::default()
                })
                .collect(),
            warnings: vec![],
        };
        for direction in [Direction::LeftRight, Direction::TopBottom] {
            let block = layered(&graph, direction);
            assert_eq!(block.positions.len(), 4);
            assert_no_overlap(&graph, &block.positions);
            assert!(block.width > 0.0 && block.height > 0.0);
        }
    }
}
