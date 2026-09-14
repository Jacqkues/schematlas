use super::{assemble, ColumnRow, RelationRow, SchemaConnector};
use crate::{
    domain::Graph,
    error::{AppError, Result},
};
use sqlx::Connection;
pub struct MysqlConnector;
#[async_trait::async_trait]
impl SchemaConnector for MysqlConnector {
    async fn inspect(&self, dsn: &str) -> Result<Graph> {
        let mut conn = super::connect_mysql(dsn).await?;
        let (database,): (Option<String>,) = sqlx::query_as("SELECT DATABASE()")
            .fetch_one(&mut conn)
            .await?;
        if database.is_none() {
            return Err(AppError::Validation(
                "Include a database name in the MySQL connection URL.".into(),
            ));
        }
        let tables = sqlx::query_as::<_,(String,String,String)>("SELECT TABLE_SCHEMA,TABLE_NAME,CASE WHEN TABLE_TYPE='VIEW' THEN 'view' ELSE 'table' END FROM information_schema.TABLES WHERE TABLE_SCHEMA NOT IN ('information_schema','mysql','performance_schema','sys') ORDER BY TABLE_SCHEMA,TABLE_NAME").fetch_all(&mut conn).await?;
        let columns = sqlx::query_as::<_,ColumnRow>("SELECT TABLE_SCHEMA,TABLE_NAME,COLUMN_NAME,COLUMN_TYPE,IS_NULLABLE,CASE WHEN COLUMN_KEY='PRI' THEN 'YES' ELSE 'NO' END,COLUMN_DEFAULT,NULLIF(COLUMN_COMMENT,'') FROM information_schema.COLUMNS WHERE TABLE_SCHEMA NOT IN ('information_schema','mysql','performance_schema','sys') ORDER BY TABLE_SCHEMA,TABLE_NAME,ORDINAL_POSITION").fetch_all(&mut conn).await?;
        let relations = sqlx::query_as::<_,RelationRow>("SELECT TABLE_SCHEMA,TABLE_NAME,COLUMN_NAME,REFERENCED_TABLE_SCHEMA,REFERENCED_TABLE_NAME,REFERENCED_COLUMN_NAME,CONSTRAINT_NAME FROM information_schema.KEY_COLUMN_USAGE WHERE TABLE_SCHEMA NOT IN ('information_schema','mysql','performance_schema','sys') AND REFERENCED_TABLE_NAME IS NOT NULL ORDER BY TABLE_SCHEMA,TABLE_NAME,CONSTRAINT_NAME,ORDINAL_POSITION").fetch_all(&mut conn).await?;
        let keys = sqlx::query_as::<_, (String, String, String, String)>("SELECT s.TABLE_SCHEMA,s.TABLE_NAME,s.INDEX_NAME,s.COLUMN_NAME FROM information_schema.STATISTICS s WHERE s.NON_UNIQUE=0 AND s.TABLE_SCHEMA NOT IN ('information_schema','mysql','performance_schema','sys') AND NOT EXISTS (SELECT 1 FROM information_schema.STATISTICS x WHERE x.TABLE_SCHEMA=s.TABLE_SCHEMA AND x.TABLE_NAME=s.TABLE_NAME AND x.INDEX_NAME=s.INDEX_NAME AND (x.COLUMN_NAME IS NULL OR x.SUB_PART IS NOT NULL)) ORDER BY s.TABLE_SCHEMA,s.TABLE_NAME,s.INDEX_NAME,s.SEQ_IN_INDEX").fetch_all(&mut conn).await?;
        conn.close().await?;
        let mut graph = assemble(tables, columns, relations);
        super::apply_unique_keys(&mut graph, keys);
        Ok(graph)
    }
}
