use super::{assemble, ColumnRow, RelationRow, SchemaConnector};
use crate::{domain::Graph, error::Result};
use sqlx::{Connection, PgConnection};
pub struct PostgresConnector;
#[async_trait::async_trait]
impl SchemaConnector for PostgresConnector {
    async fn inspect(&self, dsn: &str) -> Result<Graph> {
        let mut conn = PgConnection::connect(dsn).await?;
        sqlx::query("SET default_transaction_read_only = on")
            .execute(&mut conn)
            .await?;
        sqlx::query("SET statement_timeout = '15000'")
            .execute(&mut conn)
            .await?;
        let tables =
            sqlx::query_as::<_, (String, String, String)>(include_str!("postgres_tables.sql"))
                .fetch_all(&mut conn)
                .await?;
        let columns = sqlx::query_as::<_, ColumnRow>(include_str!("postgres_columns.sql"))
            .fetch_all(&mut conn)
            .await?;
        let relations = sqlx::query_as::<_, RelationRow>(include_str!("postgres_relations.sql"))
            .fetch_all(&mut conn)
            .await?;
        let keys = sqlx::query_as::<_, (String, String, String, String)>(include_str!(
            "postgres_unique.sql"
        ))
        .fetch_all(&mut conn)
        .await?;
        conn.close().await?;
        let mut graph = assemble(tables, columns, relations);
        super::apply_unique_keys(&mut graph, keys);
        Ok(graph)
    }
}
