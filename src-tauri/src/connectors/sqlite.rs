use super::SchemaConnector;
use crate::{
    domain::{entity_id, Entity, Field, Graph, Relation},
    error::{AppError, Result},
};
use sqlx::{sqlite::SqliteConnectOptions, Connection, Row, SqliteConnection};
pub struct SqliteConnector;
#[async_trait::async_trait]
impl SchemaConnector for SqliteConnector {
    async fn inspect(&self, path: &str) -> Result<Graph> {
        if !std::path::Path::new(path).is_file() {
            return Err(AppError::Validation(
                "Choose an existing SQLite database file.".into(),
            ));
        }
        let mut conn = SqliteConnection::connect_with(
            &SqliteConnectOptions::new().filename(path).read_only(true),
        )
        .await?;
        sqlx::query("PRAGMA query_only = ON")
            .execute(&mut conn)
            .await?;
        let rows = sqlx::query("SELECT name,type FROM sqlite_schema WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name").fetch_all(&mut conn).await?;
        let mut graph = Graph::default();
        let mut primary_keys = std::collections::HashMap::new();
        for row in &rows {
            let name: String = row.try_get("name")?;
            let kind: String = row.try_get("type")?;
            let columns = sqlx::query("SELECT name,type,\"notnull\",dflt_value,pk FROM pragma_table_xinfo(?) WHERE hidden <> 1 ORDER BY cid").bind(&name).fetch_all(&mut conn).await?;
            let mut pk: Vec<(i64, String)> = vec![];
            let fields = columns
                .into_iter()
                .map(|c| -> Result<Field> {
                    let name: String = c.try_get("name")?;
                    let ordinal: i64 = c.try_get("pk")?;
                    if ordinal > 0 {
                        pk.push((ordinal, name.clone()));
                    }
                    let primary_key = ordinal > 0;
                    let nullable = c.try_get::<i64, _>("notnull")? == 0;
                    Ok(Field {
                        name,
                        data_type: c.try_get("type")?,
                        nullable,
                        primary_key,
                        required: !nullable,
                        default_value: c.try_get("dflt_value")?,
                        description: None,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            pk.sort_by_key(|(n, _)| *n);
            let mut fields = fields;
            // A rowid alias has no separate primary-key index. This also handles
            // SQLite's distinct inline and table-level PRIMARY KEY DESC semantics.
            let (pk_indexes,): (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM pragma_index_list(?) WHERE origin = 'pk'")
                    .bind(&name)
                    .fetch_one(&mut conn)
                    .await?;
            for field in &mut fields {
                if field.primary_key
                    && pk.len() == 1
                    && pk_indexes == 0
                    && field.data_type.eq_ignore_ascii_case("INTEGER")
                {
                    field.nullable = false;
                    field.required = true;
                }
            }
            let mut unique_keys = vec![];
            if !pk.is_empty() {
                unique_keys.push(pk.iter().map(|(_, name)| name.clone()).collect());
            }
            let indexes = sqlx::query(
                "SELECT name FROM pragma_index_list(?) WHERE \"unique\"=1 AND partial=0",
            )
            .bind(&name)
            .fetch_all(&mut conn)
            .await?;
            for index in indexes {
                let index_name: String = index.try_get("name")?;
                let columns = sqlx::query("SELECT name FROM pragma_index_info(?) ORDER BY seqno")
                    .bind(index_name)
                    .fetch_all(&mut conn)
                    .await?;
                let names = columns
                    .iter()
                    .map(|column| column.try_get::<Option<String>, _>("name"))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if !names.is_empty() && names.iter().all(Option::is_some) {
                    unique_keys.push(names.into_iter().flatten().collect());
                }
            }
            primary_keys.insert(name.clone(), pk);
            graph.entities.push(Entity {
                id: entity_id("main", &name),
                name,
                namespace: "main".into(),
                kind,
                fields,
                description: None,
                method: None,
                unique_keys: Some(unique_keys),
            });
        }
        for entity in &graph.entities {
            let rows = sqlx::query("SELECT id,seq,\"table\",\"from\",\"to\" FROM pragma_foreign_key_list(?) ORDER BY id,seq").bind(&entity.name).fetch_all(&mut conn).await?;
            for row in rows {
                let table: String = row.try_get("table")?;
                let seq: i64 = row.try_get("seq")?;
                let target = entity_id("main", &table);
                if !graph.entities.iter().any(|e| e.id == target) {
                    graph.warnings.push(format!(
                        "{} references unavailable table {table}.",
                        entity.name
                    ));
                    continue;
                }
                let target_field: Option<String> =
                    row.try_get::<Option<String>, _>("to")?.or_else(|| {
                        primary_keys
                            .get(&table)
                            .and_then(|pk| pk.get(seq as usize))
                            .map(|(_, name)| name.clone())
                    });
                let id: i64 = row.try_get("id")?;
                graph.relations.push(Relation {
                    id: format!("{}-{id}-{seq}", entity.id),
                    source: entity.id.clone(),
                    target,
                    source_field: row.try_get("from")?,
                    target_field,
                    label: format!("fk_{}_{id}", entity.name),
                });
            }
        }
        conn.close().await?;
        Ok(graph)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn reads_composite_and_implicit_keys_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let mut db = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql("CREATE TABLE parent (a TEXT NOT NULL, b INTEGER NOT NULL, PRIMARY KEY(a,b)); CREATE TABLE child (x TEXT, y INTEGER, FOREIGN KEY(x,y) REFERENCES parent); CREATE VIEW summary AS SELECT a FROM parent;").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        let before = std::fs::read(&path).unwrap();
        let graph = SqliteConnector
            .inspect(path.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(graph.entities.len(), 3);
        assert_eq!(graph.relations.len(), 2);
        assert_eq!(graph.relations[0].target_field.as_deref(), Some("a"));
        assert_eq!(graph.relations[1].target_field.as_deref(), Some("b"));
        assert_eq!(before, std::fs::read(&path).unwrap());
        graph.validate().unwrap();
        assert!(SqliteConnector
            .inspect("/nonexistent/atlas.db")
            .await
            .is_err());
    }
    #[tokio::test]
    async fn inspects_only_complete_unconditional_unique_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("unique.db");
        let mut db = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql("CREATE TABLE keys (id INTEGER PRIMARY KEY, a TEXT, b TEXT, c TEXT, UNIQUE(a,b)); CREATE UNIQUE INDEX partial_key ON keys(c) WHERE c IS NOT NULL; CREATE UNIQUE INDEX expression_key ON keys(lower(c));").execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        let graph = SqliteConnector
            .inspect(path.to_str().unwrap())
            .await
            .unwrap();
        let keys = graph.entities[0].unique_keys.as_ref().unwrap();
        assert!(keys.contains(&vec!["id".to_string()]));
        assert!(keys.contains(&vec!["a".to_string(), "b".to_string()]));
        assert_eq!(keys.len(), 2);
    }
    #[tokio::test]
    async fn distinguishes_rowid_aliases_from_nullable_descending_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keys.db");
        let mut db = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::raw_sql("CREATE TABLE normal (id INTEGER PRIMARY KEY, description TEXT); CREATE TABLE descending (id INTEGER PRIMARY KEY DESC); CREATE TABLE table_level (id INTEGER, PRIMARY KEY(id DESC)); CREATE TABLE composite (a TEXT,b INTEGER,PRIMARY KEY(a,b)) WITHOUT ROWID;")
            .execute(&mut db).await.unwrap();
        db.close().await.unwrap();
        let graph = SqliteConnector
            .inspect(path.to_str().unwrap())
            .await
            .unwrap();
        for entity in graph.entities {
            let pk = entity.fields.iter().find(|f| f.primary_key).unwrap();
            assert_eq!(pk.nullable, entity.name == "descending", "{}", entity.name);
        }
    }
}
