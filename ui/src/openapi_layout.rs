//! OpenAPI maps align each route with its models, then their nested dependencies.
use crate::layout::{entity_height, NODE_WIDTH};
use crate::types::{CanvasGroup, Graph, Position};
use std::collections::{BTreeMap, HashMap, VecDeque};

const COLUMN_GAP: f64 = 180.0;
const CARD_GAP: f64 = 40.0;
const SECTION_GAP: f64 = 80.0;

pub fn is_openapi(graph: &Graph) -> bool {
    !graph.entities.is_empty()
        && graph
            .entities
            .iter()
            .all(|e| matches!(e.kind.as_str(), "operation" | "schema"))
}

/// Route rows reserve space for their complete model tree. Directly consumed
/// models stay beside their route even when another model also references them.
/// Multi-domain shared models and unused models get separate rows below routes.
/// Breadth-first assignment keeps cycles finite and runs only in the layout worker.
pub fn arrange(graph: &Graph, groups: &[CanvasGroup]) -> HashMap<String, Position> {
    let mut owners = HashMap::new();
    for group in groups {
        for id in &group.node_ids {
            owners.entry(id.as_str()).or_insert(group.name.as_str());
        }
    }
    let section = |id, namespace| owners.get(id).copied().unwrap_or(namespace);
    let mut routes: Vec<_> = graph
        .entities
        .iter()
        .filter(|e| e.kind == "operation")
        .collect();
    routes.sort_by(|a, b| {
        section(a.id.as_str(), a.namespace.as_str())
            .cmp(section(b.id.as_str(), b.namespace.as_str()))
            .then(a.name.cmp(&b.name))
            .then(a.method.cmp(&b.method))
            .then(a.id.cmp(&b.id))
    });
    let route_order: HashMap<_, _> = routes
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut models: Vec<_> = graph
        .entities
        .iter()
        .filter(|e| e.kind != "operation")
        .collect();
    models.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    let model_index: HashMap<_, _> = models
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    let mut consumers = vec![vec![]; models.len()];
    let mut children = vec![vec![]; models.len()];
    for relation in &graph.relations {
        let Some(&target) = model_index.get(relation.target.as_str()) else {
            continue;
        };
        if let Some(&route) = route_order.get(relation.source.as_str()) {
            consumers[target].push(route);
        } else if let Some(&model) = model_index.get(relation.source.as_str()) {
            children[model].push(target);
        }
    }
    for ids in consumers.iter_mut().chain(children.iter_mut()) {
        ids.sort_unstable();
        ids.dedup();
    }
    let shared_row = routes.len();
    let unused_row = shared_row + 1;
    // (route row, dependency column). Reserve every direct model before walking
    // references, so cross-domain references never displace another route's model.
    let mut slots = vec![None; models.len()];
    let mut queue = VecDeque::new();
    for (i, consumers) in consumers.iter().enumerate() {
        if let Some(&first) = consumers.first() {
            let first_route = routes[first];
            let shared = consumers.iter().any(|&r| {
                section(routes[r].id.as_str(), routes[r].namespace.as_str())
                    != section(first_route.id.as_str(), first_route.namespace.as_str())
            });
            slots[i] = Some((if shared { shared_row } else { first }, 0));
            queue.push_back(i);
        }
    }
    // Stable seed order makes ownership of equally close nested models predictable.
    queue.make_contiguous().sort_by_key(|&i| (slots[i], i));
    assign_descendants(&children, &mut slots, &mut queue);
    for i in 0..models.len() {
        if slots[i].is_none() {
            slots[i] = Some((unused_row, 0));
            queue.push_back(i);
            assign_descendants(&children, &mut slots, &mut queue);
        }
    }
    let mut rows: BTreeMap<usize, BTreeMap<usize, Vec<usize>>> = BTreeMap::new();
    for route in 0..routes.len() {
        rows.entry(route).or_default();
    }
    for (i, slot) in slots.iter().enumerate() {
        if let Some((row, column)) = slot {
            rows.entry(*row)
                .or_default()
                .entry(*column)
                .or_default()
                .push(i);
        }
    }
    let mut positions = HashMap::new();
    let mut y = 80.0;
    let mut previous_section = None;
    let model_x = if routes.is_empty() {
        60.0
    } else {
        60.0 + NODE_WIDTH + COLUMN_GAP
    };
    for (row, columns) in rows {
        let route = routes.get(row).copied();
        let current_section = route.map(|r| section(r.id.as_str(), r.namespace.as_str()));
        if y > 80.0 {
            y += if current_section.is_none() || current_section != previous_section {
                SECTION_GAP
            } else {
                CARD_GAP
            };
        }
        let mut height = route.map(entity_height).unwrap_or(0.0);
        if let Some(route) = route {
            positions.insert(route.id.clone(), Position { x: 60.0, y });
        }
        for (column, ids) in columns {
            let mut offset = 0.0;
            for i in ids {
                positions.insert(
                    models[i].id.clone(),
                    Position {
                        x: model_x + column as f64 * (NODE_WIDTH + COLUMN_GAP),
                        y: y + offset,
                    },
                );
                offset += entity_height(models[i]) + CARD_GAP;
            }
            height = height.max(offset - CARD_GAP);
        }
        y += height;
        previous_section = current_section;
    }
    positions
}

fn assign_descendants(
    children: &[Vec<usize>],
    slots: &mut [Option<(usize, usize)>],
    queue: &mut VecDeque<usize>,
) {
    while let Some(parent) = queue.pop_front() {
        let Some((row, column)) = slots[parent] else {
            continue;
        };
        for &child in &children[parent] {
            if slots[child].is_none() {
                slots[child] = Some((row, column + 1));
                queue.push_back(child);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Entity, Relation};

    fn entity(id: &str, kind: &str, namespace: &str) -> Entity {
        Entity {
            id: id.into(),
            name: id.into(),
            kind: kind.into(),
            namespace: namespace.into(),
            ..Entity::default()
        }
    }
    fn relation(source: &str, target: &str) -> Relation {
        Relation {
            id: format!("{source}-{target}"),
            source: source.into(),
            target: target.into(),
            ..Relation::default()
        }
    }
    fn graph() -> Graph {
        Graph {
            entities: vec![
                entity("/users", "operation", "Users"),
                entity("Shared", "schema", "Schemas"),
                entity("/health", "operation", "System"),
                entity("User", "schema", "Schemas"),
                entity("Unused", "schema", "Schemas"),
                entity("/orders", "operation", "Orders"),
                entity("Order", "schema", "Schemas"),
            ],
            relations: vec![
                relation("/users", "User"),
                relation("/orders", "Order"),
                relation("Order", "Shared"),
                relation("User", "Shared"),
                relation("Shared", "User"),
                relation("Shared", "Shared"),
            ],
            warnings: vec![],
        }
    }
    #[test]
    fn routes_remain_left_of_shared_cyclic_and_unused_models() {
        let graph = graph();
        let positions = arrange(&graph, &[]);
        assert_eq!(positions.len(), graph.entities.len());
        for a in &graph.entities {
            let p = positions[&a.id];
            assert!(p.x.is_finite() && p.y.is_finite());
            if a.kind == "operation" {
                assert_eq!(p.x, 60.0);
                for b in graph.entities.iter().filter(|e| e.kind == "schema") {
                    assert!(p.x + NODE_WIDTH < positions[&b.id].x);
                }
            }
            for b in graph.entities.iter().filter(|e| e.id != a.id) {
                let q = positions[&b.id];
                assert!(
                    p.x + NODE_WIDTH <= q.x
                        || q.x + NODE_WIDTH <= p.x
                        || p.y + entity_height(a) <= q.y
                        || q.y + entity_height(b) <= p.y
                );
            }
        }
        assert!(positions["/orders"].y < positions["/health"].y);
        assert!(positions["/health"].y < positions["/users"].y);
        let mut shuffled = graph.clone();
        shuffled.entities.reverse();
        shuffled.relations.reverse();
        assert_eq!(positions, arrange(&shuffled, &[]));
    }
    #[test]
    fn commerce_routes_align_with_direct_and_nested_models() {
        let project: crate::types::Project =
            serde_json::from_str(include_str!("sample.json")).unwrap();
        let graph = &project.sources[1].graph;
        let positions = arrange(graph, &[]);
        let pos = |name: &str| {
            let entity = graph.entities.iter().find(|e| e.name == name).unwrap();
            positions[&entity.id]
        };
        for (route, model) in [
            ("/products", "Product"),
            ("/customers/{id}", "Customer"),
            ("/orders", "Order"),
        ] {
            assert_eq!(pos(route).y, pos(model).y);
            assert!(pos(route).x + NODE_WIDTH < pos(model).x);
        }
        for (model, nested) in [("Customer", "Address"), ("Order", "OrderItem")] {
            assert_eq!(pos(model).y, pos(nested).y);
            assert!(pos(model).x + NODE_WIDTH < pos(nested).x);
        }
        assert_eq!(pos("Product").x, pos("Order").x);
        assert!(
            pos("Error").y
                > pos("Order").y
                    + entity_height(graph.entities.iter().find(|e| e.name == "Order").unwrap())
        );
        assert_eq!(pos("Error").x, pos("Order").x);
        let mut shuffled = graph.clone();
        shuffled.entities.reverse();
        shuffled.relations.reverse();
        assert_eq!(positions, arrange(&shuffled, &[]));
    }

    #[test]
    fn empty_route_only_and_model_only_documents_are_complete() {
        for graph in [
            Graph::default(),
            Graph {
                entities: vec![entity("/health", "operation", "System")],
                ..Graph::default()
            },
            Graph {
                entities: vec![entity("User", "schema", "Schemas")],
                ..Graph::default()
            },
        ] {
            assert_eq!(arrange(&graph, &[]).len(), graph.entities.len());
        }
        let mut mixed = graph();
        assert!(is_openapi(&mixed));
        mixed.entities.push(entity("users", "table", "public"));
        assert!(!is_openapi(&mixed));
        assert!(!is_openapi(&Graph::default()));
    }
}
