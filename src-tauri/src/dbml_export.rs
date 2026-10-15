//! A connected database's catalog as DBML, so its schema can live in the repository next to the
//! other diagrams and be refreshed without losing what people wrote around it; and back, so a
//! DBML file is drawn by the same schema canvas as a connected database.
use crate::domain::{Entity, Field, Graph, Relation};
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

/// A DBML schema as the graph the schema canvas draws. Entity ids are qualified table names, the
/// same ids the file's diagram uses, so positions and annotations line up; references become one
/// relation per column pair, from the referencing table to the referenced one.
pub fn graph(schema: &Schema) -> Graph {
    let entities = schema
        .tables
        .iter()
        .map(|t| {
            let composite_pk: Vec<&str> = t
                .indexes
                .iter()
                .filter(|i| i.primary_key)
                .flat_map(|i| i.columns.iter().map(String::as_str))
                .collect();
            let mut unique: Vec<Vec<String>> = t
                .columns
                .iter()
                .filter(|c| c.unique)
                .map(|c| vec![c.name.clone()])
                .collect();
            unique.extend(
                t.indexes
                    .iter()
                    .filter(|i| i.unique)
                    .map(|i| i.columns.clone()),
            );
            Entity {
                id: t.qualified(),
                name: t.name.clone(),
                namespace: t.schema.clone(),
                kind: "table".into(),
                description: t.note.clone().or_else(|| t.annotation.note.clone()),
                fields: t
                    .columns
                    .iter()
                    .map(|c| {
                        let primary_key = c.primary_key || composite_pk.contains(&c.name.as_str());
                        Field {
                            name: c.name.clone(),
                            data_type: c.data_type.clone(),
                            nullable: !c.not_null && !primary_key,
                            primary_key,
                            required: c.not_null || primary_key,
                            default_value: c.default.clone(),
                            description: c.note.clone(),
                        }
                    })
                    .collect(),
                method: None,
                unique_keys: Some(unique),
            }
        })
        .collect();
    let mut relations = vec![];
    for (i, r) in schema.refs.iter().enumerate() {
        let label = r
            .name
            .clone()
            .unwrap_or_else(|| format!("{} → {}", r.from.table, r.to.table));
        for (j, (from, to)) in r.from.columns.iter().zip(&r.to.columns).enumerate() {
            relations.push(Relation {
                id: format!("ref-{i}-{j}"),
                source: r.from.qualified_table(),
                target: r.to.qualified_table(),
                source_field: Some(from.clone()),
                target_field: Some(to.clone()),
                label: label.clone(),
            });
        }
    }
    Graph {
        entities,
        relations,
        warnings: vec![],
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
    fn dbml_reads_back_as_the_schema_canvas_graph() {
        let text = "Table shop.users {\n  id bigint [pk]\n  email text [not null, unique]\n}\nTable shop.memberships {\n  org_id bigint\n  user_id bigint\n  indexes {\n    (org_id, user_id) [pk]\n  }\n}\nRef: shop.memberships.user_id > shop.users.id\n";
        let g = graph(&dbml::parse(text).unwrap());
        let users = &g.entities[0];
        assert_eq!(
            (users.id.as_str(), users.namespace.as_str()),
            ("shop.users", "shop")
        );
        assert!(users.fields[0].primary_key && !users.fields[1].nullable);
        assert_eq!(users.unique_keys, Some(vec![vec!["email".to_string()]]));
        assert!(g.entities[1].fields.iter().all(|f| f.primary_key));
        let r = &g.relations[0];
        assert_eq!(
            (
                r.source.as_str(),
                r.source_field.as_deref(),
                r.target.as_str()
            ),
            ("shop.memberships", Some("user_id"), "shop.users")
        );
        g.validate().unwrap();
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
