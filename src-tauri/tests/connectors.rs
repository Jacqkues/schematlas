use schema_atlas_lib::{
    connectors,
    domain::{ConnectionRequest, DatabaseKind},
    service::Workspace,
};
use sqlx::Connection;

#[tokio::test]
async fn sqlite_project_workflow_persists_snapshots_without_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let app = Workspace::open(&path).await.unwrap();
    let p = app
        .repository
        .create(schema_atlas_lib::domain::Project::new("Integration".into(), "".into()).unwrap())
        .await
        .unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/commerce.sqlite");
    let p = app
        .connect(
            &p.id,
            "Commerce",
            ConnectionRequest {
                kind: DatabaseKind::Sqlite,
                connection_string: fixture.to_string_lossy().into(),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(p.sources[0].graph.entities.len(), 6);
    assert_eq!(p.sources[0].graph.relations.len(), 6);
    let source_id = p.sources[0].id.clone();
    let p = app.refresh(&p.id, &source_id).await.unwrap();
    assert_eq!(p.sources.len(), 1);
    let p = app
        .import(&p.id, include_str!("../../examples/commerce.openapi.json"))
        .await
        .unwrap();
    assert_eq!(p.sources.len(), 2);
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("connectionString"));
    assert!(!json.contains(&fixture.to_string_lossy().to_string()));
    drop(app);
    let app = Workspace::open(&path).await.unwrap();
    assert_eq!(app.repository.list().await.unwrap()[0].sources.len(), 2);
    assert!(matches!(
        app.refresh(&p.id, &source_id).await,
        Err(schema_atlas_lib::error::AppError::Reconnect)
    ));
    app.delete_project(&p.id).await.unwrap();
    assert!(app.repository.list().await.unwrap().is_empty());
}

#[tokio::test]
#[ignore = "Requires ATLAS_TEST_POSTGRES, an isolated disposable PostgreSQL database"]
async fn postgres_catalog_integration() {
    let dsn = std::env::var("ATLAS_TEST_POSTGRES").unwrap();
    let mut conn = sqlx::PgConnection::connect(&dsn).await.unwrap();
    sqlx::raw_sql("CREATE SCHEMA atlas_test; CREATE TABLE atlas_test.parent(a int NOT NULL,b text NOT NULL,PRIMARY KEY(a,b)); CREATE TABLE atlas_test.child(x int,y text,FOREIGN KEY(x,y) REFERENCES atlas_test.parent(a,b)); CREATE VIEW atlas_test.overview AS SELECT a FROM atlas_test.parent; CREATE SCHEMA atlas_second; CREATE TABLE atlas_second.parent(id int PRIMARY KEY); CREATE TABLE atlas_second.cross_link(a int,b text,FOREIGN KEY(a,b) REFERENCES atlas_test.parent(a,b));").execute(&mut conn).await.unwrap();
    conn.close().await.unwrap();
    let graph = connectors::inspect(&ConnectionRequest {
        kind: DatabaseKind::Postgres,
        connection_string: dsn,
    })
    .await
    .unwrap();
    assert_eq!(
        graph
            .entities
            .iter()
            .filter(|e| e.namespace == "atlas_test")
            .count(),
        3
    );
    assert_eq!(graph.relations.len(), 4);
    assert_eq!(
        graph.entities.iter().filter(|e| e.name == "parent").count(),
        2
    );
    assert!(graph
        .relations
        .iter()
        .any(|r| r.source.contains("atlas_second") && r.target.contains("atlas_test")));
    assert!(graph.entities.iter().any(|e| e.kind == "view"));
    assert!(graph
        .relations
        .iter()
        .any(|r| r.source_field.as_deref() == Some("x") && r.target_field.as_deref() == Some("a")));
    graph.validate().unwrap();
}
#[tokio::test]
#[ignore = "Requires ATLAS_TEST_MYSQL, an isolated disposable MySQL/MariaDB database"]
async fn mysql_catalog_integration() {
    let dsn = std::env::var("ATLAS_TEST_MYSQL").unwrap();
    let mut conn = sqlx::MySqlConnection::connect(&dsn).await.unwrap();
    for sql in [
        "CREATE TABLE parent(a int NOT NULL,b varchar(40) NOT NULL,PRIMARY KEY(a,b))",
        "CREATE TABLE child(x int,y varchar(40),FOREIGN KEY(x,y) REFERENCES parent(a,b))",
        "CREATE VIEW overview AS SELECT a FROM parent",
        "CREATE DATABASE atlas_second",
        "CREATE TABLE atlas_second.parent(id int PRIMARY KEY)",
        "CREATE TABLE atlas_second.cross_link(a int,b varchar(40),FOREIGN KEY(a,b) REFERENCES atlas_test.parent(a,b))",
    ] {
        sqlx::query(sql).execute(&mut conn).await.unwrap();
    }
    conn.close().await.unwrap();
    let graph = connectors::inspect(&ConnectionRequest {
        kind: DatabaseKind::Mysql,
        connection_string: dsn,
    })
    .await
    .unwrap();
    assert_eq!(graph.entities.len(), 5);
    assert_eq!(
        graph.entities.iter().filter(|e| e.name == "parent").count(),
        2
    );
    assert_eq!(graph.relations.len(), 4);
    assert!(graph
        .relations
        .iter()
        .any(|r| r.source.contains("atlas_second") && r.target.contains("atlas_test")));
    assert!(graph.entities.iter().any(|e| e.kind == "view"));
    graph.validate().unwrap();
}

#[tokio::test]
#[ignore = "Requires isolated ATLAS_TEST_POSTGRES and ATLAS_TEST_MYSQL databases"]
async fn live_query_types_and_empty_results() {
    use schema_atlas_lib::execution::sql;
    use serde_json::json;
    for (kind,dsn,query) in [
        (DatabaseKind::Postgres,std::env::var("ATLAS_TEST_POSTGRES").unwrap(),"SELECT 7::bigint AS id, 'hello'::text AS name, 12.50::numeric AS amount, DATE '2026-09-10' AS day, NULL::text AS empty"),
        (DatabaseKind::Mysql,std::env::var("ATLAS_TEST_MYSQL").unwrap(),"SELECT CAST(7 AS SIGNED) AS id, 'hello' AS name, CAST(12.50 AS DECIMAL(10,2)) AS amount, DATE '2026-09-10' AS day, NULL AS empty")
    ] {
        let request=ConnectionRequest{kind,connection_string:dsn};
        let result=sql::execute(&request,query).await.unwrap();
        assert_eq!(result.rows[0],vec![json!(7),json!("hello"),json!("12.50"),json!("2026-09-10"),serde_json::Value::Null]);
        let empty=sql::execute(&request,"SELECT 1 AS id WHERE 1=0").await.unwrap();
        assert!(empty.rows.is_empty());assert_eq!(empty.columns[0].name,"id");
    }
}
