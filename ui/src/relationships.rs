//! Cardinality labels and neighborhoods derived from the graph's foreign keys.
use crate::types::{Entity, Graph, Relation};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Relationship {
    pub id: String,
    pub source: String,
    pub target: String,
    pub source_field: Option<String>,
    pub target_field: Option<String>,
    pub label: String,
    pub source_cardinality: String,
    pub target_cardinality: String,
    pub description: String,
}

fn is_unique(entity: Option<&Entity>, columns: &[String]) -> Option<bool> {
    let entity = entity?;
    if columns.is_empty() {
        return None;
    }
    let pk: Vec<String> = entity
        .fields
        .iter()
        .filter(|f| f.primary_key)
        .map(|f| f.name.clone())
        .collect();
    let mut keys: Vec<&[String]> = entity
        .unique_keys
        .as_ref()
        .map(|keys| keys.iter().map(|k| k.as_slice()).collect())
        .unwrap_or_default();
    if !pk.is_empty() {
        keys.push(pk.as_slice());
    }
    if keys
        .iter()
        .any(|key| !key.is_empty() && key.iter().all(|name| columns.contains(name)))
    {
        return Some(true);
    }
    if entity.unique_keys.is_none() {
        None
    } else {
        Some(false)
    }
}

/// A composite foreign key is one relationship, not one relationship per column.
pub fn relationships(graph: &Graph, database: bool) -> Vec<Relationship> {
    let entities: HashMap<&str, &Entity> =
        graph.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut order: Vec<String> = Vec::new();
    let mut constraints: HashMap<String, Vec<&Relation>> = HashMap::new();
    for r in &graph.relations {
        let key = if database {
            format!("{}\u{0}{}\u{0}{}", r.source, r.target, r.label)
        } else {
            r.id.clone()
        };
        let parts = constraints.entry(key.clone()).or_insert_with(|| {
            order.push(key);
            Vec::new()
        });
        parts.push(r);
    }
    order
        .iter()
        .map(|key| {
            let parts = &constraints[key];
            let first = parts[0];
            let from = entities.get(first.source.as_str()).copied();
            let to = entities.get(first.target.as_str()).copied();
            let source_columns: Vec<String> = parts.iter().filter_map(|r| r.source_field.clone()).collect();
            let target_columns: Vec<String> = parts.iter().filter_map(|r| r.target_field.clone()).collect();
            let source_unique = is_unique(from, &source_columns);
            let target_unique = is_unique(to, &target_columns);
            let optional = source_columns.iter().any(|name| {
                from.and_then(|e| e.fields.iter().find(|f| &f.name == name))
                    .is_none_or(|f| f.nullable)
            });
            let source_cardinality = if !database {
                ""
            } else {
                match source_unique {
                    Some(true) => "0..1",
                    Some(false) => "0..N",
                    None => "0..?",
                }
            };
            let target_cardinality = if !database {
                ""
            } else {
                match (target_unique, optional) {
                    (Some(true), true) => "0..1",
                    (Some(true), false) => "1",
                    (Some(false), true) => "0..N",
                    (Some(false), false) => "1..N",
                    (None, true) => "0..?",
                    (None, false) => "1..?",
                }
            };
            let description = if database {
                format!(
                    "{} ({}) → {} ({}). {} referencing rows per referenced row; {} referenced rows per referencing row.{}",
                    from.map(|e| e.name.as_str()).unwrap_or("?"),
                    source_columns.join(", "),
                    to.map(|e| e.name.as_str()).unwrap_or("?"),
                    target_columns.join(", "),
                    source_cardinality,
                    target_cardinality,
                    if source_unique.is_none() || target_unique.is_none() {
                        " Refresh the schema to inspect unique keys; ? means unknown maximum."
                    } else {
                        ""
                    }
                )
            } else {
                first.label.clone()
            };
            let single = parts.len() == 1;
            Relationship {
                id: first.id.clone(),
                source: first.source.clone(),
                target: first.target.clone(),
                source_field: if single { first.source_field.clone() } else { None },
                target_field: if single { first.target_field.clone() } else { None },
                label: first.label.clone(),
                source_cardinality: source_cardinality.into(),
                target_cardinality: target_cardinality.into(),
                description,
            }
        })
        .collect()
}

/// The selected node plus every node it shares a relation with.
pub fn neighborhood(graph: &Graph, selected: Option<&str>) -> HashSet<String> {
    let mut ids = HashSet::new();
    let Some(selected) = selected else { return ids };
    ids.insert(selected.to_string());
    for r in &graph.relations {
        if r.source == selected || r.target == selected {
            ids.insert(r.source.clone());
            ids.insert(r.target.clone());
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Field;

    fn table(id: &str, keys: Option<Vec<Vec<String>>>) -> Entity {
        Entity {
            id: id.into(),
            name: id.into(),
            namespace: "main".into(),
            kind: "table".into(),
            unique_keys: keys,
            fields: ["a", "b"]
                .iter()
                .map(|name| Field {
                    name: (*name).into(),
                    data_type: "int".into(),
                    primary_key: id == "parent",
                    required: true,
                    ..Field::default()
                })
                .collect(),
            ..Entity::default()
        }
    }
    fn graph(keys: Option<Vec<Vec<String>>>) -> Graph {
        Graph {
            entities: vec![table("parent", None), table("child", keys)],
            relations: ["a", "b"]
                .iter()
                .map(|name| Relation {
                    id: (*name).into(),
                    source: "child".into(),
                    target: "parent".into(),
                    source_field: Some((*name).into()),
                    target_field: Some((*name).into()),
                    label: "fk_pair".into(),
                })
                .collect(),
            warnings: vec![],
        }
    }

    #[test]
    fn coalesces_composite_constraints_and_recognizes_composite_uniqueness() {
        let result = relationships(&graph(Some(vec![vec!["a".into(), "b".into()]])), true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source_cardinality, "0..1");
        assert_eq!(result[0].target_cardinality, "1");
    }
    #[test]
    fn distinguishes_nullable_many_to_one_from_one_to_one() {
        let mut g = graph(Some(vec![]));
        g.entities[1].fields[1].nullable = true;
        let r = &relationships(&g, true)[0];
        assert_eq!(r.source_cardinality, "0..N");
        assert_eq!(r.target_cardinality, "0..1");
    }
    #[test]
    fn does_not_invent_uniqueness_for_older_snapshots_or_openapi_refs() {
        assert_eq!(
            relationships(&graph(None), true)[0].source_cardinality,
            "0..?"
        );
        assert_eq!(relationships(&graph(None), false).len(), 2);
        assert_eq!(relationships(&graph(None), false)[0].source_cardinality, "");
    }
    #[test]
    fn neighborhood_includes_linked_nodes_only() {
        let mut g = graph(None);
        g.entities.push(table("unrelated", None));
        let mut ids: Vec<String> = neighborhood(&g, Some("parent")).into_iter().collect();
        ids.sort();
        assert_eq!(ids, vec!["child", "parent"]);
        assert!(neighborhood(&g, None).is_empty());
    }
}
