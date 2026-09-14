mod mssql;
mod mysql;
mod mysql_connection;
mod postgres;
mod sqlite;
use crate::{
    domain::{ConnectionRequest, DatabaseKind, Graph},
    error::{AppError, Result},
};
use async_trait::async_trait;

pub(crate) use mysql_connection::connect as connect_mysql;

/// Implement this port to add another catalog without changing the UI graph model.
#[async_trait]
pub trait SchemaConnector: Send + Sync {
    async fn inspect(&self, connection_string: &str) -> Result<Graph>;
}

pub async fn inspect(request: &ConnectionRequest) -> Result<Graph> {
    if request.connection_string.trim().is_empty() || request.connection_string.len() > 8192 {
        return Err(AppError::Validation(
            "Provide a connection string (up to 8 KB).".into(),
        ));
    }
    let connector: Box<dyn SchemaConnector> = match request.kind {
        DatabaseKind::Sqlite => Box::new(sqlite::SqliteConnector),
        DatabaseKind::Postgres => Box::new(postgres::PostgresConnector),
        DatabaseKind::Mysql | DatabaseKind::Mariadb => Box::new(mysql::MysqlConnector),
        DatabaseKind::Mssql => Box::new(mssql::MssqlConnector),
    };
    let graph = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        connector.inspect(&request.connection_string),
    )
    .await
    .map_err(|_| AppError::Timeout)??;
    graph.validate()?;
    Ok(graph)
}
// Normalized SQL catalog rows: namespace, table, column, type, nullable, primary key, default, comment.
type ColumnRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
);
type RelationRow = (String, String, String, String, String, String, String);
fn assemble(
    tables: Vec<(String, String, String)>,
    columns: Vec<ColumnRow>,
    relations: Vec<RelationRow>,
) -> Graph {
    use crate::domain::{entity_id, Entity, Field, Relation};
    let mut entities: Vec<Entity> = tables
        .into_iter()
        .map(|(namespace, name, kind)| Entity {
            id: entity_id(&namespace, &name),
            name,
            namespace,
            kind,
            fields: vec![],
            description: None,
            method: None,
            unique_keys: None,
        })
        .collect();
    let lookup: std::collections::HashMap<_, _> = entities
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.clone(), i))
        .collect();
    for (ns, table, name, data_type, nullable, pk, default_value, description) in columns {
        if let Some(&i) = lookup.get(&entity_id(&ns, &table)) {
            entities[i].fields.push(Field {
                name,
                data_type,
                nullable: nullable == "YES",
                primary_key: pk == "YES",
                required: nullable != "YES",
                default_value,
                description,
            });
        }
    }
    let mut warnings = vec![];
    let relations = relations
        .into_iter()
        .enumerate()
        .filter_map(|(i, (ns, t, c, tns, tt, tc, label))| {
            let source = entity_id(&ns, &t);
            let target = entity_id(&tns, &tt);
            if !lookup.contains_key(&source) || !lookup.contains_key(&target) {
                warnings.push(format!(
                    "{label}: a referenced table is outside the visible catalog."
                ));
                return None;
            }
            Some(Relation {
                id: format!("fk-{i}"),
                source,
                target,
                source_field: Some(c),
                target_field: Some(tc),
                label,
            })
        })
        .collect();
    Graph {
        entities,
        relations,
        warnings,
    }
}

/// Apply only unconditional, valid unique indexes from a connector's catalog.
fn apply_unique_keys(graph: &mut Graph, rows: Vec<(String, String, String, String)>) {
    let mut keys = std::collections::BTreeMap::<(String, String), Vec<String>>::new();
    for (namespace, table, index, column) in rows {
        keys.entry((crate::domain::entity_id(&namespace, &table), index))
            .or_default()
            .push(column);
    }
    for entity in &mut graph.entities {
        entity.unique_keys = Some(
            keys.iter()
                .filter(|((id, _), _)| id == &entity.id)
                .map(|(_, columns)| columns.clone())
                .collect(),
        );
    }
}
