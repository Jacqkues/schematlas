use super::{assemble, SchemaConnector};
use crate::{domain::Graph, error::Result};
use tokio_util::compat::TokioAsyncWriteCompatExt;
pub struct MssqlConnector;
#[async_trait::async_trait]
impl SchemaConnector for MssqlConnector {
    async fn inspect(&self, dsn: &str) -> Result<Graph> {
        let config = tiberius::Config::from_ado_string(dsn)?;
        let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
        tcp.set_nodelay(true)?;
        let mut client = tiberius::Client::connect(config, tcp.compat_write()).await?;
        let rows = client.simple_query("SELECT s.name AS ns,o.name,CASE WHEN o.type='V' THEN 'view' ELSE 'table' END AS kind FROM sys.objects o JOIN sys.schemas s ON s.schema_id=o.schema_id WHERE o.type IN ('U','V') AND o.is_ms_shipped=0 ORDER BY s.name,o.name").await?.into_first_result().await?;
        let tables = rows
            .iter()
            .map(|r| (get(r, 0), get(r, 1), get(r, 2)))
            .collect();
        let rows = client
            .simple_query(include_str!("mssql_columns.sql"))
            .await?
            .into_first_result()
            .await?;
        let columns = rows
            .iter()
            .map(|r| {
                (
                    get(r, 0),
                    get(r, 1),
                    get(r, 2),
                    get(r, 3),
                    get(r, 4),
                    get(r, 5),
                    r.get::<&str, _>(6).map(str::to_owned),
                    r.get::<&str, _>(7).map(str::to_owned),
                )
            })
            .collect();
        let rows = client.simple_query("SELECT ss.name,st.name,sc.name,ts.name,tt.name,tc.name,fk.name FROM sys.foreign_key_columns f JOIN sys.foreign_keys fk ON fk.object_id=f.constraint_object_id JOIN sys.tables st ON st.object_id=f.parent_object_id JOIN sys.schemas ss ON ss.schema_id=st.schema_id JOIN sys.columns sc ON sc.object_id=st.object_id AND sc.column_id=f.parent_column_id JOIN sys.tables tt ON tt.object_id=f.referenced_object_id JOIN sys.schemas ts ON ts.schema_id=tt.schema_id JOIN sys.columns tc ON tc.object_id=tt.object_id AND tc.column_id=f.referenced_column_id ORDER BY ss.name,st.name,fk.name,f.constraint_column_id").await?.into_first_result().await?;
        let relations = rows
            .iter()
            .map(|r| {
                (
                    get(r, 0),
                    get(r, 1),
                    get(r, 2),
                    get(r, 3),
                    get(r, 4),
                    get(r, 5),
                    get(r, 6),
                )
            })
            .collect();
        let rows = client.simple_query("SELECT s.name,t.name,i.name,c.name FROM sys.indexes i JOIN sys.tables t ON t.object_id=i.object_id JOIN sys.schemas s ON s.schema_id=t.schema_id JOIN sys.index_columns k ON k.object_id=i.object_id AND k.index_id=i.index_id JOIN sys.columns c ON c.object_id=k.object_id AND c.column_id=k.column_id WHERE i.is_unique=1 AND i.is_disabled=0 AND i.is_hypothetical=0 AND i.has_filter=0 AND k.key_ordinal>0 ORDER BY s.name,t.name,i.name,k.key_ordinal").await?.into_first_result().await?;
        let keys = rows
            .iter()
            .map(|r| (get(r, 0), get(r, 1), get(r, 2), get(r, 3)))
            .collect();
        let mut graph = assemble(tables, columns, relations);
        super::apply_unique_keys(&mut graph, keys);
        Ok(graph)
    }
}
fn get(row: &tiberius::Row, index: usize) -> String {
    row.get::<&str, _>(index).unwrap_or_default().to_owned()
}
