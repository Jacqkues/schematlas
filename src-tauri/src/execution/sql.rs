//! SQL execution is called only after a reviewed tool request is approved.
use crate::{
    domain::{ConnectionRequest, DatabaseKind},
    error::{AppError, Result},
};
use base64::Engine;
use futures_util::TryStreamExt;
use serde::Serialize;
use serde_json::{json, Value};
use sqlparser::{
    ast::{ObjectType, Statement},
    dialect::{Dialect, MsSqlDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect},
    parser::Parser,
};
use sqlx::{Column, Connection, Executor, Row, TypeInfo, ValueRef};
use std::time::Duration;
use tokio_util::compat::TokioAsyncWriteCompatExt;
const MAX_ROWS: usize = 200;
const MAX_BYTES: usize = 1024 * 1024;
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOutput {
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<Vec<Value>>,
    pub rows_affected: u64,
    pub truncated: bool,
    pub warnings: Vec<String>,
}
#[derive(Serialize)]
pub struct QueryColumn {
    pub name: String,
    pub data_type: String,
}

pub fn validate(kind: &DatabaseKind, sql: &str) -> Result<()> {
    if sql.is_empty() || sql.len() > 64 * 1024 {
        return Err(AppError::Validation(
            "SQL must contain 1–65,536 bytes.".into(),
        ));
    }
    let dialect: Box<dyn Dialect> = match kind {
        DatabaseKind::Postgres => Box::new(PostgreSqlDialect {}),
        DatabaseKind::Mysql | DatabaseKind::Mariadb => Box::new(MySqlDialect {}),
        DatabaseKind::Sqlite => Box::new(SQLiteDialect {}),
        DatabaseKind::Mssql => Box::new(MsSqlDialect {}),
    };
    let statements = Parser::parse_sql(dialect.as_ref(), sql).map_err(|_| {
        AppError::Validation("SQL could not be parsed for this database dialect.".into())
    })?;
    if statements.len() != 1 {
        return Err(AppError::Validation(
            "Submit exactly one SQL statement per tool call.".into(),
        ));
    }
    let supported = match &statements[0] {
        Statement::Drop { object_type, .. } => matches!(
            object_type,
            ObjectType::Table | ObjectType::View | ObjectType::Index
        ),
        statement => matches!(
            statement,
            Statement::Query(_)
                | Statement::Insert(_)
                | Statement::Update { .. }
                | Statement::Delete(_)
                | Statement::CreateTable(_)
                | Statement::AlterTable { .. }
                | Statement::Truncate { .. }
        ),
    };
    if !supported {
        return Err(AppError::Validation("Use a query or table-level data/DDL statement. Session control, file attachment, and administrative commands are not supported.".into()));
    }
    Ok(())
}
impl QueryOutput {
    fn push(&mut self, row: Vec<Value>, bytes: &mut usize) -> bool {
        *bytes += serde_json::to_vec(&row)
            .map(|v| v.len())
            .unwrap_or(MAX_BYTES);
        if self.rows.len() >= MAX_ROWS || *bytes > MAX_BYTES {
            self.truncated = true;
            return false;
        }
        self.rows.push(row);
        true
    }
}
macro_rules! try_value {
    ($row:expr,$index:expr,$ty:ty) => {
        if let Ok(value) = $row.try_get::<Option<$ty>, _>($index) {
            return json!(value);
        }
    };
}
macro_rules! try_text {
    ($row:expr,$index:expr,$ty:ty) => {
        if let Ok(value) = $row.try_get::<Option<$ty>, _>($index) {
            return value
                .map(|v| Value::String(v.to_string()))
                .unwrap_or(Value::Null);
        }
    };
}
fn pg_cell(row: &sqlx::postgres::PgRow, i: usize) -> Value {
    if row.try_get_raw(i).is_ok_and(|v| v.is_null()) {
        return Value::Null;
    }
    try_value!(row, i, bool);
    try_value!(row, i, i16);
    try_value!(row, i, i32);
    try_value!(row, i, i64);
    try_value!(row, i, f32);
    try_value!(row, i, f64);
    try_value!(row, i, String);
    try_value!(row, i, Value);
    try_text!(row, i, sqlx::types::BigDecimal);
    try_text!(row, i, uuid::Uuid);
    try_text!(row, i, chrono::NaiveDate);
    try_text!(row, i, chrono::NaiveTime);
    try_text!(row, i, chrono::NaiveDateTime);
    try_text!(row, i, chrono::DateTime<chrono::Utc>);
    if let Ok(value) = row.try_get::<Vec<u8>, _>(i) {
        return json!({"base64":base64::engine::general_purpose::STANDARD.encode(value)});
    }
    json!({"unavailableType":row.columns()[i].type_info().name()})
}
fn mysql_cell(row: &sqlx::mysql::MySqlRow, i: usize) -> Value {
    if row.try_get_raw(i).is_ok_and(|v| v.is_null()) {
        return Value::Null;
    }
    try_value!(row, i, i8);
    try_value!(row, i, i16);
    try_value!(row, i, i32);
    try_value!(row, i, i64);
    try_value!(row, i, u64);
    try_value!(row, i, f32);
    try_value!(row, i, f64);
    try_value!(row, i, String);
    try_value!(row, i, Value);
    try_text!(row, i, sqlx::types::BigDecimal);
    try_text!(row, i, chrono::NaiveDate);
    try_text!(row, i, chrono::NaiveTime);
    try_text!(row, i, chrono::NaiveDateTime);
    if let Ok(value) = row.try_get::<Vec<u8>, _>(i) {
        return json!({"base64":base64::engine::general_purpose::STANDARD.encode(value)});
    }
    json!({"unavailableType":row.columns()[i].type_info().name()})
}
fn sqlite_cell(row: &sqlx::sqlite::SqliteRow, i: usize) -> Value {
    if row.try_get_raw(i).is_ok_and(|v| v.is_null()) {
        return Value::Null;
    }
    try_value!(row, i, i64);
    try_value!(row, i, f64);
    try_value!(row, i, String);
    if let Ok(value) = row.try_get::<Vec<u8>, _>(i) {
        return json!({"base64":base64::engine::general_purpose::STANDARD.encode(value)});
    }
    json!({"unavailableType":row.columns()[i].type_info().name()})
}
macro_rules! sqlx_run {
    ($conn:expr,$sql:expr,$decode:ident) => {{
        let mut output = QueryOutput::default();
        let metadata = (&mut $conn).describe($sql).await?;
        output.columns = metadata
            .columns()
            .iter()
            .map(|c| QueryColumn {
                name: c.name().into(),
                data_type: c.type_info().name().into(),
            })
            .collect();
        let mut bytes = 0;
        {
            let mut stream = sqlx::raw_sql($sql).fetch_many(&mut $conn);
            while let Some(item) = stream.try_next().await? {
                match item {
                    sqlx::Either::Left(done) => output.rows_affected += done.rows_affected(),
                    sqlx::Either::Right(row) => {
                        if output.columns.is_empty() {
                            output.columns = row
                                .columns()
                                .iter()
                                .map(|c| QueryColumn {
                                    name: c.name().into(),
                                    data_type: c.type_info().name().into(),
                                })
                                .collect();
                        }
                        if !output.push(
                            (0..row.len()).map(|i| $decode(&row, i)).collect(),
                            &mut bytes,
                        ) {
                            break;
                        }
                    }
                }
            }
        }
        $conn.close().await?;
        Ok(output)
    }};
}
pub async fn execute(request: &ConnectionRequest, sql: &str) -> Result<QueryOutput> {
    validate(&request.kind, sql)?;
    tokio::time::timeout(Duration::from_secs(30), execute_inner(request, sql))
        .await
        .map_err(|_| AppError::Timeout)?
}
async fn execute_inner(request: &ConnectionRequest, sql: &str) -> Result<QueryOutput> {
    match request.kind {
        DatabaseKind::Postgres => {
            let mut conn = sqlx::PgConnection::connect(&request.connection_string).await?;
            sqlx::query("SET statement_timeout = '20000'")
                .execute(&mut conn)
                .await?;
            sqlx_run!(conn, sql, pg_cell)
        }
        DatabaseKind::Mysql | DatabaseKind::Mariadb => {
            let mut conn = sqlx::MySqlConnection::connect(&request.connection_string).await?;
            sqlx_run!(conn, sql, mysql_cell)
        }
        DatabaseKind::Sqlite => {
            let path = std::path::Path::new(&request.connection_string);
            if !path.is_file() {
                return Err(AppError::Validation(
                    "The SQLite file no longer exists.".into(),
                ));
            }
            let mut conn = sqlx::SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(false)
                    .foreign_keys(true)
                    .busy_timeout(Duration::from_secs(5)),
            )
            .await?;
            sqlx_run!(conn, sql, sqlite_cell)
        }
        DatabaseKind::Mssql => {
            let config = tiberius::Config::from_ado_string(&request.connection_string)?;
            let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true)?;
            let mut client = tiberius::Client::connect(config, tcp.compat_write()).await?;
            let mut stream = client.simple_query(sql).await?;
            let mut output = QueryOutput::default();
            let mut bytes = 0;
            while let Some(item) = stream.try_next().await? {
                if let tiberius::QueryItem::Row(row) = item {
                    if output.columns.is_empty() {
                        output.columns = row
                            .columns()
                            .iter()
                            .map(|c| QueryColumn {
                                name: c.name().into(),
                                data_type: format!("{:?}", c.column_type()),
                            })
                            .collect();
                    }
                    if !output.push(row.into_iter().map(mssql_cell).collect(), &mut bytes) {
                        break;
                    }
                }
            }
            output.warnings.push(
                "SQL Server does not report affected-row counts through this result stream.".into(),
            );
            Ok(output)
        }
    }
}
fn mssql_cell(value: tiberius::ColumnData<'static>) -> Value {
    use tiberius::ColumnData::*;
    match value {
        U8(v) => json!(v),
        I16(v) => json!(v),
        I32(v) => json!(v),
        I64(v) => json!(v),
        F32(v) => json!(v),
        F64(v) => json!(v),
        Bit(v) => json!(v),
        String(v) => json!(v),
        Guid(v) => json!(v.map(|s| s.to_string())),
        Numeric(v) => json!(v.map(|s| s.to_string())),
        Binary(v) => json!(v.map(|s| base64::engine::general_purpose::STANDARD.encode(s))),
        other => json!({"nativeValue":format!("{other:?}")}),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restricts_statement_count_and_session_escape() {
        assert!(validate(&DatabaseKind::Postgres, "DROP DATABASE production").is_err());
        assert!(validate(&DatabaseKind::Postgres, "DROP TABLE old_items").is_ok());
        assert!(validate(&DatabaseKind::Sqlite, "SELECT 1; DROP TABLE x").is_err());
        assert!(validate(&DatabaseKind::Sqlite, "ATTACH 'x.db' AS extra").is_err());
        assert!(validate(
            &DatabaseKind::Postgres,
            "WITH x AS (SELECT 1) SELECT * FROM x"
        )
        .is_ok());
    }
    #[tokio::test]
    async fn query_results_and_writes_are_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.db");
        let mut conn = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await
        .unwrap();
        sqlx::query("CREATE TABLE items(id INTEGER PRIMARY KEY,name TEXT)")
            .execute(&mut conn)
            .await
            .unwrap();
        conn.close().await.unwrap();
        let request = ConnectionRequest {
            kind: DatabaseKind::Sqlite,
            connection_string: path.to_string_lossy().into(),
        };
        let empty = execute(&request, "SELECT id,name FROM items WHERE 1=0")
            .await
            .unwrap();
        assert!(empty.rows.is_empty());
        assert_eq!(empty.columns.len(), 2);
        assert_eq!(
            execute(&request, "INSERT INTO items(name) VALUES ('hello')")
                .await
                .unwrap()
                .rows_affected,
            1
        );
        assert_eq!(
            execute(&request, "SELECT id,name,NULL AS missing FROM items")
                .await
                .unwrap()
                .rows[0],
            vec![json!(1), json!("hello"), Value::Null]
        );
        let result=execute(&request,"WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n<500) SELECT n FROM seq").await.unwrap();
        assert_eq!(result.rows.len(), 200);
        assert!(result.truncated);
    }
}
