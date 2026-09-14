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
/// Hard cap on the rows one execution may return.
pub const MAX_ROWS: usize = 200;
/// Row cap used when a caller does not ask for one.
pub const DEFAULT_ROWS: usize = 50;
const MAX_BYTES: usize = 1024 * 1024;
/// Longest rendered cell before `QueryOutput::render` cuts it.
const MAX_CELL_CHARS: usize = 200;
/// Clamp a requested row limit to 1..=MAX_ROWS; None becomes DEFAULT_ROWS.
pub fn clamp_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(rows) => rows.clamp(1, MAX_ROWS),
        None => DEFAULT_ROWS,
    }
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOutput {
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<Vec<Value>>,
    pub rows_affected: u64,
    pub truncated: bool,
    pub warnings: Vec<String>,
    /// Wall-clock duration of the whole execution, connection included.
    pub elapsed_ms: u64,
    /// Row cap this execution ran with.
    pub limit: usize,
}
#[derive(Serialize)]
pub struct QueryColumn {
    pub name: String,
    pub data_type: String,
}

fn dialect_for(kind: &DatabaseKind) -> Box<dyn Dialect> {
    match kind {
        DatabaseKind::Postgres => Box::new(PostgreSqlDialect {}),
        DatabaseKind::Mysql | DatabaseKind::Mariadb => Box::new(MySqlDialect {}),
        DatabaseKind::Sqlite => Box::new(SQLiteDialect {}),
        DatabaseKind::Mssql => Box::new(MsSqlDialect {}),
    }
}
pub fn validate(kind: &DatabaseKind, sql: &str) -> Result<()> {
    if sql.is_empty() || sql.len() > 64 * 1024 {
        return Err(AppError::Validation(
            "SQL must contain 1–65,536 bytes.".into(),
        ));
    }
    let dialect = dialect_for(kind);
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
                // Read-only plans, including SQLite's EXPLAIN QUERY PLAN.
                | Statement::Explain { .. }
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
        if self.rows.len() >= self.limit || *bytes > MAX_BYTES {
            self.truncated = true;
            return false;
        }
        self.rows.push(row);
        true
    }
    /// Compact text table for agents: a header row, a `---` separator, one line per row with cells
    /// joined by ` | `, then a summary line and one `note: ` line per warning. A write with neither
    /// columns nor rows renders only the summary.
    pub fn render(&self) -> String {
        let mut lines: Vec<String> = Vec::with_capacity(self.rows.len() + self.warnings.len() + 3);
        if !self.columns.is_empty() {
            lines.push(
                self.columns
                    .iter()
                    .map(|column| column.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" | "),
            );
            lines.push("---".into());
        }
        for row in &self.rows {
            lines.push(row.iter().map(render_cell).collect::<Vec<_>>().join(" | "));
        }
        let mut summary = format!("{} rows", self.rows.len());
        if self.truncated {
            summary.push_str(&format!(" (truncated at {})", self.limit));
        }
        if self.rows_affected > 0 {
            summary.push_str(&format!(", {} affected", self.rows_affected));
        }
        summary.push_str(&format!(", {} ms", self.elapsed_ms));
        lines.push(summary);
        lines.extend(self.warnings.iter().map(|note| format!("note: {note}")));
        lines.join("\n")
    }
}
/// One cell: NULL as `NULL`, text verbatim with line breaks folded so a row stays on one line, and
/// anything else as compact JSON. Long cells are cut so one wide column cannot flood the context.
fn render_cell(value: &Value) -> String {
    let text = match value {
        Value::Null => "NULL".to_string(),
        Value::String(text) => text.replace("\r\n", "⏎").replace(['\n', '\r'], "⏎"),
        other => other.to_string(),
    };
    match text.char_indices().nth(MAX_CELL_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text,
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
    ($conn:expr,$sql:expr,$decode:ident,$limit:expr) => {{
        let mut output = QueryOutput {
            limit: $limit,
            ..QueryOutput::default()
        };
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
/// Execute one validated statement under the hard row cap.
pub async fn execute(request: &ConnectionRequest, sql: &str) -> Result<QueryOutput> {
    execute_with(request, sql, MAX_ROWS).await
}
/// Execute one validated statement with a row cap; `elapsed_ms` measures the whole execution.
pub async fn execute_with(
    request: &ConnectionRequest,
    sql: &str,
    limit: usize,
) -> Result<QueryOutput> {
    validate(&request.kind, sql)?;
    let limit = clamp_limit(Some(limit));
    let started = std::time::Instant::now();
    let mut output =
        tokio::time::timeout(Duration::from_secs(30), execute_inner(request, sql, limit))
            .await
            .map_err(|_| AppError::Timeout)??;
    output.elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(output)
}
async fn execute_inner(
    request: &ConnectionRequest,
    sql: &str,
    limit: usize,
) -> Result<QueryOutput> {
    match request.kind {
        DatabaseKind::Postgres => {
            let mut conn = sqlx::PgConnection::connect(&request.connection_string).await?;
            sqlx::query("SET statement_timeout = '20000'")
                .execute(&mut conn)
                .await?;
            sqlx_run!(conn, sql, pg_cell, limit)
        }
        DatabaseKind::Mysql | DatabaseKind::Mariadb => {
            let mut conn = crate::connectors::connect_mysql(&request.connection_string).await?;
            sqlx_run!(conn, sql, mysql_cell, limit)
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
            sqlx_run!(conn, sql, sqlite_cell, limit)
        }
        DatabaseKind::Mssql => {
            let config = tiberius::Config::from_ado_string(&request.connection_string)?;
            let tcp = tokio::net::TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true)?;
            let mut client = tiberius::Client::connect(config, tcp.compat_write()).await?;
            let mut stream = client.simple_query(sql).await?;
            let mut output = QueryOutput {
                limit,
                ..QueryOutput::default()
            };
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
/// One statement prepared for review: the exact SQL that will run, plus a caveat for the user.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub sql: String,
    pub note: Option<String>,
}
/// Table and schema names reach these builders from catalog inspection, but they are never
/// interpolated raw: every name is checked here and then quoted or escaped for the dialect.
fn check_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(AppError::Validation(
            "Table and schema names must not be empty.".into(),
        ));
    }
    if name.chars().count() > 128 {
        return Err(AppError::Validation(
            "Table and schema names must be 128 characters or fewer.".into(),
        ));
    }
    if name.contains('\0') {
        return Err(AppError::Validation(
            "Table and schema names must not contain NUL.".into(),
        ));
    }
    Ok(())
}
/// Quote one identifier for this dialect, doubling the closing quote character inside it.
fn quote_name(kind: &DatabaseKind, name: &str) -> Result<String> {
    check_name(name)?;
    Ok(match kind {
        DatabaseKind::Postgres | DatabaseKind::Sqlite => {
            format!("\"{}\"", name.replace('"', "\"\""))
        }
        DatabaseKind::Mysql | DatabaseKind::Mariadb => format!("`{}`", name.replace('`', "``")),
        DatabaseKind::Mssql => format!("[{}]", name.replace(']', "]]")),
    })
}
/// Quote one name as a string literal for catalog lookups. MySQL also reads a backslash as an
/// escape inside literals, so it is doubled there.
fn quote_literal(kind: &DatabaseKind, name: &str) -> Result<String> {
    check_name(name)?;
    let escaped = name.replace('\'', "''");
    Ok(match kind {
        DatabaseKind::Mysql | DatabaseKind::Mariadb => {
            format!("'{}'", escaped.replace('\\', "\\\\"))
        }
        _ => format!("'{escaped}'"),
    })
}
/// `ns.table`, unless the namespace is implicit: empty everywhere, or SQLite's `main`.
fn qualify(kind: &DatabaseKind, namespace: &str, table: &str) -> Result<String> {
    let table = quote_name(kind, table)?;
    let implicit = namespace.is_empty()
        || (matches!(kind, DatabaseKind::Sqlite) && namespace.eq_ignore_ascii_case("main"));
    if implicit {
        return Ok(table);
    }
    Ok(format!("{}.{table}", quote_name(kind, namespace)?))
}
/// Row count and on-disk size for one table, from catalog statistics where they are available:
/// counting every row of a large table is far too expensive for a review-and-run tool.
pub fn table_stats(kind: &DatabaseKind, namespace: &str, table: &str) -> Result<Prepared> {
    let estimate = "Row count and size are estimates from catalog statistics.";
    let (sql, note) = match kind {
        DatabaseKind::Postgres => {
            let schema = if namespace.is_empty() {
                "current_schema()".to_string()
            } else {
                quote_literal(kind, namespace)?
            };
            (
                format!(
                    "SELECT n.nspname AS table_schema, c.relname AS table_name, \
                     c.reltuples::bigint AS estimated_rows, \
                     pg_total_relation_size(c.oid) AS total_bytes, \
                     pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size \
                     FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE c.relname = {} AND n.nspname = {schema}",
                    quote_literal(kind, table)?
                ),
                estimate,
            )
        }
        DatabaseKind::Mysql | DatabaseKind::Mariadb => {
            let schema = if namespace.is_empty() {
                "DATABASE()".to_string()
            } else {
                quote_literal(kind, namespace)?
            };
            (
                format!(
                    "SELECT TABLE_SCHEMA AS table_schema, TABLE_NAME AS table_name, \
                     TABLE_ROWS AS estimated_rows, DATA_LENGTH AS data_bytes, \
                     INDEX_LENGTH AS index_bytes, DATA_LENGTH + INDEX_LENGTH AS total_bytes \
                     FROM information_schema.TABLES \
                     WHERE TABLE_NAME = {} AND TABLE_SCHEMA = {schema}",
                    quote_literal(kind, table)?
                ),
                estimate,
            )
        }
        DatabaseKind::Sqlite => (
            format!(
                "SELECT COUNT(*) AS row_count FROM {}",
                qualify(kind, namespace, table)?
            ),
            "Row count is exact; COUNT(*) can be slow on a large table.",
        ),
        DatabaseKind::Mssql => {
            let schema = if namespace.is_empty() {
                "SCHEMA_NAME()".to_string()
            } else {
                quote_literal(kind, namespace)?
            };
            (
                format!(
                    "SELECT s.name AS table_schema, t.name AS table_name, \
                     SUM(CASE WHEN p.index_id IN (0, 1) THEN p.row_count ELSE 0 END) AS estimated_rows, \
                     SUM(p.reserved_page_count) * 8 AS reserved_kb, \
                     SUM(p.used_page_count) * 8 AS used_kb \
                     FROM sys.dm_db_partition_stats p \
                     JOIN sys.tables t ON t.object_id = p.object_id \
                     JOIN sys.schemas s ON s.schema_id = t.schema_id \
                     WHERE t.name = {} AND s.name = {schema} GROUP BY s.name, t.name",
                    quote_literal(kind, table)?
                ),
                estimate,
            )
        }
    };
    Ok(Prepared {
        sql,
        note: Some(note.into()),
    })
}
/// A first look at the data in one table, capped the same way execution is.
pub fn sample_rows(
    kind: &DatabaseKind,
    namespace: &str,
    table: &str,
    limit: usize,
) -> Result<Prepared> {
    let target = qualify(kind, namespace, table)?;
    let limit = clamp_limit(Some(limit));
    let sql = match kind {
        DatabaseKind::Mssql => format!("SELECT TOP ({limit}) * FROM {target}"),
        _ => format!("SELECT * FROM {target} LIMIT {limit}"),
    };
    Ok(Prepared { sql, note: None })
}
/// A read-only execution plan for one SELECT statement. The statement is parsed first, so nothing
/// but a query can be wrapped in EXPLAIN.
pub fn explain(kind: &DatabaseKind, sql: &str) -> Result<Prepared> {
    if matches!(kind, DatabaseKind::Mssql) {
        // SHOWPLAN needs session-level SET statements, which this one-statement runner cannot send.
        return Err(AppError::Validation(
            "Execution plans are not available for SQL Server in Schematlas.".into(),
        ));
    }
    let statement = sql.trim().trim_end_matches(';').trim_end();
    let rejected = || AppError::Validation("EXPLAIN accepts one SELECT statement.".into());
    if statement.is_empty() || statement.len() > 64 * 1024 {
        return Err(rejected());
    }
    let parsed =
        Parser::parse_sql(dialect_for(kind).as_ref(), statement).map_err(|_| rejected())?;
    if parsed.len() != 1 || !matches!(parsed[0], Statement::Query(_)) {
        return Err(rejected());
    }
    let sql = match kind {
        DatabaseKind::Postgres => format!("EXPLAIN (FORMAT TEXT) {statement}"),
        DatabaseKind::Sqlite => format!("EXPLAIN QUERY PLAN {statement}"),
        _ => format!("EXPLAIN {statement}"),
    };
    Ok(Prepared { sql, note: None })
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
    /// Open a throwaway SQLite file holding one table, and return a request pointing at it.
    async fn sqlite_fixture(dir: &tempfile::TempDir, rows: &[&str]) -> ConnectionRequest {
        let path = dir.path().join("tools.db");
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
        for name in rows {
            sqlx::query("INSERT INTO items(name) VALUES (?)")
                .bind(name)
                .execute(&mut conn)
                .await
                .unwrap();
        }
        conn.close().await.unwrap();
        ConnectionRequest {
            kind: DatabaseKind::Sqlite,
            connection_string: path.to_string_lossy().into(),
        }
    }
    #[test]
    fn clamps_requested_row_limits() {
        assert_eq!(clamp_limit(None), DEFAULT_ROWS);
        assert_eq!(clamp_limit(Some(0)), 1);
        assert_eq!(clamp_limit(Some(1)), 1);
        assert_eq!(clamp_limit(Some(25)), 25);
        assert_eq!(clamp_limit(Some(MAX_ROWS)), MAX_ROWS);
        assert_eq!(clamp_limit(Some(MAX_ROWS + 1)), MAX_ROWS);
        assert_eq!(clamp_limit(Some(usize::MAX)), MAX_ROWS);
    }
    #[tokio::test]
    async fn row_limits_apply_per_call() {
        let dir = tempfile::tempdir().unwrap();
        let request = sqlite_fixture(&dir, &[]).await;
        let sql = "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n<500) SELECT n FROM seq";
        let limited = execute_with(&request, sql, 10).await.unwrap();
        assert_eq!(limited.rows.len(), 10);
        assert_eq!(limited.limit, 10);
        assert!(limited.truncated);
        assert!(limited.elapsed_ms < 30_000);
        let serialized = serde_json::to_value(&limited).unwrap();
        assert!(serialized.get("elapsedMs").is_some());
        assert_eq!(serialized["limit"], json!(10));
        // A caller asking for no rows still gets a usable answer instead of an empty one.
        assert_eq!(execute_with(&request, sql, 0).await.unwrap().rows.len(), 1);
        let capped = execute(&request, sql).await.unwrap();
        assert_eq!(capped.limit, MAX_ROWS);
        assert_eq!(capped.rows.len(), MAX_ROWS);
    }
    #[test]
    fn renders_compact_text_for_agents() {
        let long = "x".repeat(250);
        let rows = QueryOutput {
            columns: vec![
                QueryColumn {
                    name: "id".into(),
                    data_type: "INTEGER".into(),
                },
                QueryColumn {
                    name: "name".into(),
                    data_type: "TEXT".into(),
                },
            ],
            rows: vec![vec![json!(1), Value::Null], vec![json!(2), json!(long)]],
            elapsed_ms: 7,
            limit: 50,
            ..Default::default()
        };
        assert_eq!(
            rows.render(),
            format!(
                "id | name\n---\n1 | NULL\n2 | {}\u{2026}\n2 rows, 7 ms",
                "x".repeat(200)
            )
        );
        let mixed = QueryOutput {
            columns: vec![QueryColumn {
                name: "note".into(),
                data_type: "TEXT".into(),
            }],
            rows: vec![vec![json!("first\nsecond")], vec![json!({"a": [1, true]})]],
            truncated: true,
            warnings: vec!["partial result".into()],
            elapsed_ms: 3,
            limit: 2,
            ..Default::default()
        };
        assert_eq!(
            mixed.render(),
            "note\n---\nfirst\u{23ce}second\n{\"a\":[1,true]}\n2 rows (truncated at 2), 3 ms\nnote: partial result"
        );
        let write = QueryOutput {
            rows_affected: 4,
            elapsed_ms: 12,
            limit: 50,
            ..Default::default()
        };
        assert_eq!(write.render(), "0 rows, 4 affected, 12 ms");
    }
    #[test]
    fn prepares_sample_rows_for_every_dialect() {
        assert_eq!(
            sample_rows(&DatabaseKind::Postgres, "public", "items", 5)
                .unwrap()
                .sql,
            "SELECT * FROM \"public\".\"items\" LIMIT 5"
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Mysql, "shop", "items", 5)
                .unwrap()
                .sql,
            "SELECT * FROM `shop`.`items` LIMIT 5"
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Mariadb, "shop", "items", 5_000)
                .unwrap()
                .sql,
            format!("SELECT * FROM `shop`.`items` LIMIT {MAX_ROWS}")
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Sqlite, "main", "items", 5)
                .unwrap()
                .sql,
            "SELECT * FROM \"items\" LIMIT 5"
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Sqlite, "extra", "items", 5)
                .unwrap()
                .sql,
            "SELECT * FROM \"extra\".\"items\" LIMIT 5"
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Mssql, "dbo", "items", 5)
                .unwrap()
                .sql,
            "SELECT TOP (5) * FROM [dbo].[items]"
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Postgres, "", "weird-name", 3)
                .unwrap()
                .sql,
            "SELECT * FROM \"weird-name\" LIMIT 3"
        );
        assert_eq!(
            sample_rows(&DatabaseKind::Mssql, "weird schema", "weird-name", 3)
                .unwrap()
                .sql,
            "SELECT TOP (3) * FROM [weird schema].[weird-name]"
        );
        for kind in [
            DatabaseKind::Postgres,
            DatabaseKind::Mysql,
            DatabaseKind::Mariadb,
            DatabaseKind::Sqlite,
            DatabaseKind::Mssql,
        ] {
            let prepared = sample_rows(&kind, "weird-schema", "weird-name", 3).unwrap();
            assert!(prepared.note.is_none());
            assert!(validate(&kind, &prepared.sql).is_ok(), "{}", prepared.sql);
        }
        assert!(matches!(
            sample_rows(&DatabaseKind::Postgres, "", "", 5),
            Err(AppError::Validation(_))
        ));
        assert!(sample_rows(&DatabaseKind::Postgres, "", &"t".repeat(129), 5).is_err());
        assert!(sample_rows(&DatabaseKind::Postgres, "", &"t".repeat(128), 5).is_ok());
        assert!(sample_rows(&DatabaseKind::Postgres, "", "bad\0name", 5).is_err());
        assert!(sample_rows(&DatabaseKind::Postgres, "bad\0schema", "items", 5).is_err());
    }
    #[test]
    fn prepares_table_stats_for_every_dialect() {
        let estimate = Some("Row count and size are estimates from catalog statistics.");
        let postgres = table_stats(&DatabaseKind::Postgres, "public", "items").unwrap();
        assert!(postgres
            .sql
            .starts_with("SELECT n.nspname AS table_schema, c.relname AS table_name, c.reltuples::bigint AS estimated_rows, "));
        assert!(postgres
            .sql
            .contains("pg_total_relation_size(c.oid) AS total_bytes, pg_size_pretty(pg_total_relation_size(c.oid)) AS total_size "));
        assert!(postgres
            .sql
            .contains("FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace "));
        assert!(postgres
            .sql
            .ends_with("WHERE c.relname = 'items' AND n.nspname = 'public'"));
        assert_eq!(postgres.note.as_deref(), estimate);
        assert!(table_stats(&DatabaseKind::Postgres, "", "items")
            .unwrap()
            .sql
            .ends_with("AND n.nspname = current_schema()"));
        let mysql = table_stats(&DatabaseKind::Mysql, "shop", "items").unwrap();
        assert!(mysql.sql.contains(
            "TABLE_ROWS AS estimated_rows, DATA_LENGTH AS data_bytes, INDEX_LENGTH AS index_bytes, DATA_LENGTH + INDEX_LENGTH AS total_bytes "
        ));
        assert!(mysql.sql.contains("FROM information_schema.TABLES "));
        assert!(mysql
            .sql
            .ends_with("WHERE TABLE_NAME = 'items' AND TABLE_SCHEMA = 'shop'"));
        assert_eq!(mysql.note.as_deref(), estimate);
        assert!(table_stats(&DatabaseKind::Mariadb, "", "items")
            .unwrap()
            .sql
            .ends_with("AND TABLE_SCHEMA = DATABASE()"));
        assert_eq!(
            table_stats(&DatabaseKind::Sqlite, "main", "items")
                .unwrap()
                .sql,
            "SELECT COUNT(*) AS row_count FROM \"items\""
        );
        assert_eq!(
            table_stats(&DatabaseKind::Sqlite, "extra", "weird-name")
                .unwrap()
                .sql,
            "SELECT COUNT(*) AS row_count FROM \"extra\".\"weird-name\""
        );
        assert_eq!(
            table_stats(&DatabaseKind::Sqlite, "", "items")
                .unwrap()
                .note
                .as_deref(),
            Some("Row count is exact; COUNT(*) can be slow on a large table.")
        );
        let mssql = table_stats(&DatabaseKind::Mssql, "dbo", "items").unwrap();
        assert!(mssql.sql.contains(
            "SUM(CASE WHEN p.index_id IN (0, 1) THEN p.row_count ELSE 0 END) AS estimated_rows, "
        ));
        assert!(mssql.sql.contains(
            "SUM(p.reserved_page_count) * 8 AS reserved_kb, SUM(p.used_page_count) * 8 AS used_kb "
        ));
        assert!(mssql.sql.contains(
            "FROM sys.dm_db_partition_stats p JOIN sys.tables t ON t.object_id = p.object_id JOIN sys.schemas s ON s.schema_id = t.schema_id "
        ));
        assert!(mssql
            .sql
            .ends_with("WHERE t.name = 'items' AND s.name = 'dbo' GROUP BY s.name, t.name"));
        assert_eq!(mssql.note.as_deref(), estimate);
        assert!(table_stats(&DatabaseKind::Mssql, "", "items")
            .unwrap()
            .sql
            .contains("AND s.name = SCHEMA_NAME()"));
        for kind in [
            DatabaseKind::Postgres,
            DatabaseKind::Mysql,
            DatabaseKind::Mariadb,
            DatabaseKind::Sqlite,
            DatabaseKind::Mssql,
        ] {
            let prepared = table_stats(&kind, "weird-schema", "weird-name").unwrap();
            assert!(prepared.note.is_some());
            assert!(validate(&kind, &prepared.sql).is_ok(), "{}", prepared.sql);
        }
        assert!(matches!(
            table_stats(&DatabaseKind::Postgres, "public", ""),
            Err(AppError::Validation(_))
        ));
        assert!(table_stats(&DatabaseKind::Sqlite, "", "bad\0name").is_err());
    }
    #[test]
    fn hostile_names_are_quoted_never_interpolated() {
        let evil = "x\"; DROP TABLE y; --";
        for (kind, expected) in [
            (
                DatabaseKind::Postgres,
                "SELECT * FROM \"x\"\"; DROP TABLE y; --\" LIMIT 5",
            ),
            (
                DatabaseKind::Sqlite,
                "SELECT * FROM \"x\"\"; DROP TABLE y; --\" LIMIT 5",
            ),
            (
                DatabaseKind::Mysql,
                "SELECT * FROM `x\"; DROP TABLE y; --` LIMIT 5",
            ),
            (
                DatabaseKind::Mssql,
                "SELECT TOP (5) * FROM [x\"; DROP TABLE y; --]",
            ),
        ] {
            let prepared = sample_rows(&kind, "", evil, 5).unwrap();
            assert_eq!(prepared.sql, expected);
            // Quoting has to leave exactly one statement behind.
            assert!(validate(&kind, &prepared.sql).is_ok(), "{}", prepared.sql);
        }
        assert_eq!(
            sample_rows(&DatabaseKind::Mssql, "", "x]; DROP TABLE y; --", 5)
                .unwrap()
                .sql,
            "SELECT TOP (5) * FROM [x]]; DROP TABLE y; --]"
        );
        let quoted = table_stats(&DatabaseKind::Postgres, "x'; DROP TABLE y; --", "items").unwrap();
        assert!(quoted
            .sql
            .ends_with("WHERE c.relname = 'items' AND n.nspname = 'x''; DROP TABLE y; --'"));
        assert!(validate(&DatabaseKind::Postgres, &quoted.sql).is_ok());
        // MySQL reads a backslash as an escape inside literals, so it must be doubled too.
        assert!(
            table_stats(&DatabaseKind::Mysql, "", "x\\'; DROP TABLE y; --")
                .unwrap()
                .sql
                .contains("WHERE TABLE_NAME = 'x\\\\''; DROP TABLE y; --'")
        );
        assert_eq!(
            table_stats(&DatabaseKind::Sqlite, "", "x\"; DROP TABLE y; --")
                .unwrap()
                .sql,
            "SELECT COUNT(*) AS row_count FROM \"x\"\"; DROP TABLE y; --\""
        );
    }
    #[test]
    fn explains_only_single_select_statements() {
        assert_eq!(
            explain(&DatabaseKind::Postgres, "SELECT 1").unwrap().sql,
            "EXPLAIN (FORMAT TEXT) SELECT 1"
        );
        assert_eq!(
            explain(&DatabaseKind::Mysql, " SELECT 1 ").unwrap().sql,
            "EXPLAIN SELECT 1"
        );
        assert_eq!(
            explain(&DatabaseKind::Mariadb, "SELECT 1").unwrap().sql,
            "EXPLAIN SELECT 1"
        );
        assert_eq!(
            explain(&DatabaseKind::Sqlite, "SELECT 1;").unwrap().sql,
            "EXPLAIN QUERY PLAN SELECT 1"
        );
        let cte = explain(
            &DatabaseKind::Postgres,
            "WITH x AS (SELECT 1) SELECT * FROM x",
        )
        .unwrap();
        assert_eq!(
            cte.sql,
            "EXPLAIN (FORMAT TEXT) WITH x AS (SELECT 1) SELECT * FROM x"
        );
        assert!(cte.note.is_none());
        // Prepared plans have to survive the same validation an agent statement goes through.
        assert!(validate(&DatabaseKind::Postgres, &cte.sql).is_ok());
        assert!(validate(&DatabaseKind::Sqlite, "EXPLAIN QUERY PLAN SELECT 1").is_ok());
        assert!(validate(&DatabaseKind::Mysql, "EXPLAIN SELECT 1").is_ok());
        for sql in [
            "UPDATE items SET name = 'x'",
            "DELETE FROM items",
            "SELECT 1; SELECT 2",
            "CREATE TABLE t(id INT)",
            "not sql at all",
            "   ",
        ] {
            assert!(
                matches!(
                    explain(&DatabaseKind::Postgres, sql),
                    Err(AppError::Validation(_))
                ),
                "{sql}"
            );
        }
        assert!(explain(&DatabaseKind::Mssql, "SELECT 1").is_err());
    }
    #[tokio::test]
    async fn sqlite_dba_tools_run_and_render() {
        let dir = tempfile::tempdir().unwrap();
        let request = sqlite_fixture(&dir, &["first", "second"]).await;
        let sample = sample_rows(&DatabaseKind::Sqlite, "main", "items", 1).unwrap();
        assert_eq!(sample.sql, "SELECT * FROM \"items\" LIMIT 1");
        let rendered = execute_with(&request, &sample.sql, 1)
            .await
            .unwrap()
            .render();
        assert!(
            rendered.starts_with("id | name\n---\n1 | first\n"),
            "{rendered}"
        );
        assert!(rendered.ends_with(" ms"), "{rendered}");
        let stats = table_stats(&DatabaseKind::Sqlite, "main", "items").unwrap();
        let rendered = execute_with(&request, &stats.sql, clamp_limit(None))
            .await
            .unwrap()
            .render();
        assert!(rendered.starts_with("row_count\n---\n2\n"), "{rendered}");
        let plan = explain(&DatabaseKind::Sqlite, &sample.sql).unwrap();
        let output = execute_with(&request, &plan.sql, clamp_limit(None))
            .await
            .unwrap();
        assert!(!output.columns.is_empty());
        assert!(output.render().contains(" ms"));
    }
}
