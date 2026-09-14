//! OpenAPI maps read from routes to models, then through model dependencies.
use crate::layout::{entity_height, NODE_WIDTH};
use crate::smart_layout::{layered, Direction};
use crate::types::{CanvasGroup, Graph, Position};
use std::collections::{HashMap, HashSet};

pub fn is_openapi(graph: &Graph) -> bool {
    !graph.entities.is_empty()
        && graph
            .entities
            .iter()
            .all(|e| matches!(e.kind.as_str(), "operation" | "schema"))
}

/// Keep every operation in the left lane, including routes without responses.
/// Model references are layered separately so shared, recursive, and unused
/// models cannot pull routes into the middle of the map. Explicit group names
/// take precedence over tags when ordering routes within the lane.
pub fn arrange(graph: &Graph, groups: &[CanvasGroup]) -> HashMap<String, Position> {
    let mut owners = HashMap::new();
    for group in groups {
        for id in &group.node_ids {
            owners.entry(id.as_str()).or_insert(group.name.as_str());
        }
    }
    let mut routes: Vec<_> = graph
        .entities
        .iter()
        .filter(|e| e.kind == "operation")
        .collect();
    let section = |id, namespace| owners.get(id).copied().unwrap_or(namespace);
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
    let mut consumers: HashMap<&str, usize> = HashMap::new();
    for relation in &graph.relations {
        if let Some(&index) = route_order.get(relation.source.as_str()) {
            consumers
                .entry(relation.target.as_str())
                .and_modify(|v| *v = (*v).min(index))
                .or_insert(index);
        }
    }
    let mut models: Vec<_> = graph
        .entities
        .iter()
        .filter(|e| e.kind != "operation")
        .cloned()
        .collect();
    models.sort_by(|a, b| {
        consumers
            .get(a.id.as_str())
            .unwrap_or(&usize::MAX)
            .cmp(consumers.get(b.id.as_str()).unwrap_or(&usize::MAX))
            .then(a.name.cmp(&b.name))
            .then(a.id.cmp(&b.id))
    });
    let model_ids: HashSet<_> = models.iter().map(|e| e.id.as_str()).collect();
    let relations = graph
        .relations
        .iter()
        .filter(|r| model_ids.contains(r.source.as_str()) && model_ids.contains(r.target.as_str()))
        .cloned()
        .collect();
    let model_graph = Graph {
        entities: models,
        relations,
        warnings: vec![],
    };
    let model_block = layered(&model_graph, Direction::LeftRight);
    let mut positions = HashMap::new();
    let mut y = 0.0;
    let mut previous = None;
    for route in &routes {
        let group = section(route.id.as_str(), route.namespace.as_str());
        if previous.is_some_and(|name| name != group) {
            y += 48.0;
        }
        positions.insert(route.id.clone(), Position { x: 60.0, y });
        y += entity_height(route) + 48.0;
        previous = Some(group);
    }
    let route_height = if routes.is_empty() { 0.0 } else { y - 48.0 };
    let height = route_height.max(model_block.height);
    for position in positions.values_mut() {
        position.y += 80.0 + (height - route_height) / 2.0;
    }
    let model_x = if routes.is_empty() {
        60.0
    } else {
        60.0 + NODE_WIDTH + 220.0
    };
    for (id, position) in model_block.positions {
        positions.insert(
            id,
            Position {
                x: model_x + position.x,
                y: 80.0 + position.y + (height - model_block.height) / 2.0,
            },
        );
    }
    positions
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
