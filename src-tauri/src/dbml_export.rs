//! A connected database's catalog as DBML, so its schema can live in the repository next to the
//! other diagrams and be refreshed without losing what people wrote around it.
use crate::domain::{Graph, Relation};
use schematlas_parser::dbml::{Cardinality, Column, Endpoint, Index, Ref, Schema, Table};
use std::collections::BTreeMap;

/// The DBML schema of a database graph. Composite foreign keys arrive as one relation per column
/// pair sharing a label; they become one ref with several columns.
pub fn schema(graph: &Graph) -> Schema {
    let mut tables = vec![];
    for entity in &graph.entities {
        let primary: Vec<&str> = entity
            .fields
            .iter()
            .filter(|f| f.primary_key)
            .map(|f| f.name.as_str())
            .collect();
        let composite = primary.len() > 1;
        let single_unique: Vec<&str> = entity
            .unique_keys
            .iter()
            .flatten()
            .filter(|k| k.len() == 1)
            .map(|k| k[0].as_str())
            .collect();
        let columns = entity
            .fields
            .iter()
            .map(|f| Column {
                name: f.name.clone(),
                data_type: f.data_type.clone(),
                primary_key: f.primary_key && !composite,
                unique: single_unique.contains(&f.name.as_str()) && !f.primary_key,
                not_null: !f.nullable && !f.primary_key,
                increment: false,
                default: f.default_value.as_deref().map(literal),
                note: f.description.clone(),
            })
            .collect();
        let mut indexes = vec![];
        if composite {
            indexes.push(Index {
                columns: primary.iter().map(|c| (*c).to_owned()).collect(),
                primary_key: true,
                ..Index::default()
            });
        }
        indexes.extend(
            entity
                .unique_keys
                .iter()
                .flatten()
                .filter(|k| k.len() > 1)
                .map(|k| Index {
                    columns: k.clone(),
                    unique: true,
                    ..Index::default()
                }),
        );
        tables.push(Table {
            schema: entity.namespace.clone(),
            name: entity.name.clone(),
            note: entity.description.clone(),
            columns,
            indexes,
            ..Table::default()
        });
    }
    let by_id: BTreeMap<&str, &Table> = graph
        .entities
        .iter()
        .zip(&tables)
        .map(|(e, t)| (e.id.as_str(), t))
        .collect();
    // (source, target, label) → column pairs, in catalog order.
    let mut keys: Vec<(&str, &str, &str)> = vec![];
    let mut pairs: BTreeMap<(&str, &str, &str), Vec<&Relation>> = BTreeMap::new();
    for r in &graph.relations {
        let key = (r.source.as_str(), r.target.as_str(), r.label.as_str());
        if !pairs.contains_key(&key) {
            keys.push(key);
        }
        pairs.entry(key).or_default().push(r);
    }
    let refs = keys
        .into_iter()
        .filter_map(|key| {
            let (from, to) = (by_id.get(key.0)?, by_id.get(key.1)?);
            let rels = &pairs[&key];
            let from_columns: Option<Vec<&str>> =
                rels.iter().map(|r| r.source_field.as_deref()).collect();
            let to_columns: Option<Vec<&str>> =
                rels.iter().map(|r| r.target_field.as_deref()).collect();
            Some(Ref {
                name: None,
                from: Endpoint::new(&from.schema, &from.name, &from_columns?),
                to: Endpoint::new(&to.schema, &to.name, &to_columns?),
                cardinality: Cardinality::ManyToOne,
                on_delete: None,
                on_update: None,
            })
        })
        .collect();
    Schema {
        tables,
        refs,
        ..Schema::default()
    }
}

/// A catalog default as a DBML literal: numbers and booleans stay bare, quoted strings become
/// DBML strings, and anything else (functions, casts) is an expression.
fn literal(value: &str) -> String {
    let v = value.trim();
    if v.parse::<f64>().is_ok() && !v.starts_with('-') || matches!(v, "true" | "false" | "null") {
        return v.to_owned();
    }
    if let Some(inner) = v
        .strip_prefix('\'')
        .and_then(|s| s.split_once('\''))
        .filter(|(_, rest)| rest.is_empty() || rest.starts_with("::"))
        .map(|(inner, _)| inner)
    {
        return format!("'{}'", inner.replace('\\', "\\\\"));
    }
    format!("`{}`", v.replace('`', ""))
}

/// A repository path for a source's DBML file.
pub fn default_path(source_name: &str) -> String {
    let slug: String = source_name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    let slug = if slug.is_empty() { "database" } else { &slug };
    format!(".schematlas/schema/{slug}.dbml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{entity_id, Entity, Field};
    use schematlas_parser::dbml;

    fn field(name: &str, ty: &str, pk: bool, nullable: bool) -> Field {
        Field {
            name: name.into(),
            data_type: ty.into(),
            nullable,
            primary_key: pk,
            ..Field::default()
        }
    }
    fn entity(name: &str, fields: Vec<Field>, unique: Vec<Vec<&str>>) -> Entity {
        Entity {
            id: entity_id("public", name),
            name: name.into(),
            namespace: "public".into(),
            kind: "table".into(),
            description: None,
            fields,
            method: None,
            unique_keys: Some(
                unique
                    .into_iter()
                    .map(|k| k.into_iter().map(str::to_owned).collect())
                    .collect(),
            ),
        }
    }
    fn relation(from: &str, to: &str, sf: &str, tf: &str, label: &str) -> Relation {
        Relation {
            id: format!("{from}-{sf}"),
            source: entity_id("public", from),
            target: entity_id("public", to),
            source_field: Some(sf.into()),
            target_field: Some(tf.into()),
            label: label.into(),
        }
    }

    #[test]
    fn database_graphs_become_dbml_that_reads_back() {
        let mut email = field("email", "text", false, false);
        email.default_value = Some("'none'::text".into());
        let mut created = field("created_at", "timestamptz", false, true);
        created.default_value = Some("now()".into());
        let graph = Graph {
            entities: vec![
                entity(
                    "users",
                    vec![field("id", "bigint", true, false), email, created],
                    vec![vec!["email"]],
                ),
                entity(
                    "memberships",
                    vec![
                        field("org_id", "bigint", true, false),
                        field("user_id", "bigint", true, false),
                    ],
                    vec![],
                ),
                entity(
                    "grants",
                    vec![
                        field("org_id", "bigint", false, false),
                        field("user_id", "bigint", false, false),
                    ],
                    vec![vec!["org_id", "user_id"]],
                ),
            ],
            relations: vec![
                relation("memberships", "users", "user_id", "id", "fk_user"),
                relation("grants", "memberships", "org_id", "org_id", "fk_m"),
                relation("grants", "memberships", "user_id", "user_id", "fk_m"),
            ],
            warnings: vec![],
        };
        let schema = schema(&graph);
        let text = dbml::write(&schema);
        assert!(
            text.contains("email text [not null, unique, default: 'none']"),
            "{text}"
        );
        assert!(text.contains("default: `now()`"), "{text}");
        assert!(text.contains("(org_id, user_id) [pk]"), "{text}");
        assert!(text.contains("(org_id, user_id) [unique]"), "{text}");
        let read = dbml::parse(&text).unwrap();
        assert_eq!(dbml::diff(&schema, &read), vec![]);
        assert_eq!(read.refs.len(), 2);
        assert_eq!(read.refs[1].from.columns, ["org_id", "user_id"]);
    }

    #[test]
    fn literals_and_paths() {
        assert_eq!(literal("42"), "42");
        assert_eq!(literal("-1"), "`-1`");
        assert_eq!(literal("'a\\b'"), "'a\\\\b'");
        assert_eq!(
            literal("nextval('s'::regclass)"),
            "`nextval('s'::regclass)`"
        );
        assert_eq!(
            default_path("Prod DB (eu)"),
            ".schematlas/schema/prod-db-eu.dbml"
        );
        assert_eq!(default_path("!!"), ".schematlas/schema/database.dbml");
    }
}
