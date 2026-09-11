use super::*;
use crate::domain::{ConnectionRequest, DatabaseKind, Project};
use sqlx::{Connection, Row};
async fn setup() -> (tempfile::TempDir, Arc<AgentHub>, String, Arc<Session>) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Arc::new(
        Workspace::open(&dir.path().join("workspace.db"))
            .await
            .unwrap(),
    );
    let project = workspace
        .repository
        .create(Project::new("ACP test".into(), String::new()).unwrap())
        .await
        .unwrap();
    let hub = AgentHub::start(workspace).await.unwrap();
    hub.connect(
        project.id.clone(),
        AgentConfig {
            executable: std::env::var("ATLAS_TEST_PYTHON")
                .unwrap_or_else(|_| "/usr/bin/python3".into()),
            args: vec![format!(
                "{}/../examples/mock-acp-agent.py",
                env!("CARGO_MANIFEST_DIR")
            )],
            cwd: dir.path().to_string_lossy().into(),
        },
    )
    .await
    .unwrap();
    let session = hub.get(&project.id).await.unwrap();
    (dir, hub, project.id, session)
}
async fn review(session: &Session) -> types::Review {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(review) = session.snapshot.lock().await.reviews.first() {
                return review.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn acp_streams_permissions_and_cancellation() {
    let (_dir, hub, id, session) = setup().await;
    session.prompt("hello".into()).await.unwrap();
    assert!(session
        .snapshot
        .lock()
        .await
        .messages
        .last()
        .unwrap()
        .text
        .contains("ACP streaming works"));
    let s = session.clone();
    let prompt = tokio::spawn(async move { s.prompt("permission".into()).await });
    let request = review(&session).await;
    assert_eq!(request.kind, "agent");
    assert!(session
        .decide(&request.id, Some("invented".into()))
        .await
        .is_err());
    session
        .decide(&request.id, Some("reject".into()))
        .await
        .unwrap();
    prompt.await.unwrap().unwrap();
    assert!(session
        .snapshot
        .lock()
        .await
        .messages
        .last()
        .unwrap()
        .text
        .contains("reject"));
    let s = session.clone();
    let prompt = tokio::spawn(async move { s.prompt("wait".into()).await });
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if session.snapshot.lock().await.status == "running" {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    session.cancel().await.unwrap();
    prompt.await.unwrap().unwrap();
    assert_eq!(session.snapshot.lock().await.status, "ready");
    hub.disconnect(&id).await;
    assert!(hub.authorized(&session.token).await.is_err());
}
#[tokio::test]
async fn project_tools_require_approval_and_revoke_tokens() {
    let (dir, hub, id, session) = setup().await;
    let path = dir.path().join("data.db");
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE items(id INTEGER)")
        .execute(&mut db)
        .await
        .unwrap();
    let project = hub
        .workspace
        .connect(
            &id,
            "Disposable SQLite",
            ConnectionRequest {
                kind: DatabaseKind::Sqlite,
                connection_string: path.to_string_lossy().into(),
            },
            None,
        )
        .await
        .unwrap();
    let source_id = project.sources[0].id.clone();
    assert!(hub
        .tool("bad-token", "list_sources", json!({}))
        .await
        .is_err());
    assert_eq!(
        hub.tool(&session.token, "get_schema", json!({"source_id":source_id}))
            .await
            .unwrap()["entities"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(hub
        .tool(
            &session.token,
            "get_schema",
            json!({"source_id":"outside-project"})
        )
        .await
        .is_err());
    for allow in [false, true] {
        let h = hub.clone();
        let token = session.token.clone();
        let args = json!({"source_id":source_id,"sql":"INSERT INTO items VALUES (7)"});
        let call = tokio::spawn(async move { h.tool(&token, "query_sql", args).await });
        let request = review(&session).await;
        let count: i64 = sqlx::query("SELECT COUNT(*) FROM items")
            .fetch_one(&mut db)
            .await
            .unwrap()
            .get(0);
        assert_eq!(count, 0);
        assert_eq!(request.details["sql"], "INSERT INTO items VALUES (7)");
        session
            .decide(
                &request.id,
                Some(if allow { "allow" } else { "reject" }.into()),
            )
            .await
            .unwrap();
        assert_eq!(call.await.unwrap().is_ok(), allow);
    }
    let count: i64 = sqlx::query("SELECT COUNT(*) FROM items")
        .fetch_one(&mut db)
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 1);
    let h = hub.clone();
    let token = session.token.clone();
    let call = tokio::spawn(async move {
        h.tool(
            &token,
            "query_sql",
            json!({"source_id":source_id,"sql":"DELETE FROM items"}),
        )
        .await
    });
    review(&session).await;
    hub.disconnect(&id).await;
    assert!(call.await.unwrap().is_err());
    assert!(hub
        .tool(&session.token, "list_sources", json!({}))
        .await
        .is_err());
    db.close().await.unwrap();
}
#[tokio::test]
async fn http_requires_approval_and_does_not_follow_redirects() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (_dir, hub, id, session) = setup().await;
    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/customers/{id}",
        axum::routing::get(move || {
            let c = c.clone();
            async move {
                c.fetch_add(1, Ordering::SeqCst);
                axum::response::Redirect::temporary("/unapproved")
            }
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let project = hub
        .workspace
        .import(&id, include_str!("../../../examples/commerce.openapi.json"))
        .await
        .unwrap();
    let source = &project.sources[0];
    let operation = source
        .graph
        .entities
        .iter()
        .find(|e| e.name == "/customers/{id}" && e.method.as_deref() == Some("GET"))
        .unwrap();
    hub.workspace
        .configure_api(
            &id,
            &source.id,
            http::ApiConnection {
                base_url: format!("http://{address}"),
                headers: Default::default(),
            },
        )
        .await
        .unwrap();
    for allow in [false, true] {
        let h = hub.clone();
        let token = session.token.clone();
        let args =
            json!({"source_id":source.id,"operation_id":operation.id,"path_parameters":{"id":"1"}});
        let call = tokio::spawn(async move { h.tool(&token, "request_http", args).await });
        let request = review(&session).await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        session
            .decide(
                &request.id,
                Some(if allow { "allow" } else { "reject" }.into()),
            )
            .await
            .unwrap();
        let result = call.await.unwrap();
        if allow {
            assert_eq!(result.unwrap()["status"], 307);
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(count.load(Ordering::SeqCst), 1);
    hub.shutdown().await;
    server.abort();
}

#[tokio::test]
async fn canvas_edits_are_scoped_persisted_and_undoable() {
    let (dir, hub, id, session) = setup().await;
    let project = hub
        .workspace
        .import(&id, include_str!("../../../examples/commerce.openapi.json"))
        .await
        .unwrap();
    let source = &project.sources[0];
    let source_id = source.id.clone();
    let a = source.graph.entities[0].id.clone();
    let b = source.graph.entities[1].id.clone();
    let mut changes = hub.changes.subscribe();
    hub.tool(&session.token,"move_nodes",json!({"source_id":source_id,"nodes":[{"node_id":a,"x":120,"y":200},{"node_id":b,"x":600,"y":200}]})).await.unwrap();
    assert_eq!(changes.recv().await.unwrap().id, id);
    assert!(hub.tool(&session.token,"move_nodes",json!({"source_id":source_id,"nodes":[{"node_id":a,"x":800,"y":400},{"node_id":"missing","x":1,"y":2}]})).await.is_err());
    let canvas = hub
        .tool(&session.token, "get_canvas", json!({"source_id":source_id}))
        .await
        .unwrap();
    assert_eq!(canvas["positions"][&a]["x"].as_f64(), Some(120.0));
    hub.tool(
        &session.token,
        "create_group",
        json!({"source_id":source_id,"name":"Customers","node_ids":[a,b],"color":"#6D9DE3"}),
    )
    .await
    .unwrap();
    let project = hub.workspace.repository.get(&id).await.unwrap();
    assert_eq!(project.sources[0].groups[0].name, "Customers");
    assert_eq!(project.sources[0].groups[0].color, "#6d9de3");
    let group_id = project.sources[0].groups[0].id.clone();
    assert!(hub.tool(&session.token, "create_group", json!({"source_id":source_id,"group_id":group_id,"name":"Invalid color","node_ids":[a,b],"color":"red; opacity: 0"})).await.is_err());
    assert_eq!(
        hub.workspace.repository.get(&id).await.unwrap().sources[0].groups[0].name,
        "Customers"
    );
    hub.tool(&session.token,"create_group",json!({"source_id":source_id,"group_id":group_id,"name":"Customer domain","node_ids":[a,b]})).await.unwrap();
    assert_eq!(
        hub.workspace.repository.get(&id).await.unwrap().sources[0].groups[0].color,
        "#6d9de3"
    );
    hub.workspace
        .repository
        .mutate(&id, |p| crate::canvas::undo(p.source_mut(&source_id)?))
        .await
        .unwrap();
    assert_eq!(
        hub.workspace.repository.get(&id).await.unwrap().sources[0].groups[0].name,
        "Customers"
    );
    assert!(hub
        .tool(
            &session.token,
            "create_group",
            json!({"source_id":"another-project-source","name":"No","node_ids":[a]})
        )
        .await
        .is_err());
    let reopened = Workspace::open(&dir.path().join("workspace.db"))
        .await
        .unwrap();
    let saved = reopened.repository.get(&id).await.unwrap();
    assert_eq!(saved.sources[0].groups.len(), 1);
    assert_eq!(saved.sources[0].groups[0].color, "#6d9de3");
    let old_group: crate::domain::CanvasGroup =
        serde_json::from_value(json!({"id":"old","name":"Existing group","nodeIds":[a]})).unwrap();
    assert_eq!(old_group.color, "#879b91");
    assert_eq!(saved.sources[0].positions[&a].x, 120.0);
    hub.tool(
        &session.token,
        "remove_group",
        json!({"source_id":source_id,"group_id":group_id}),
    )
    .await
    .unwrap();
    assert!(hub.workspace.repository.get(&id).await.unwrap().sources[0]
        .groups
        .is_empty());
    hub.shutdown().await;
}

#[tokio::test]
async fn acp_activity_is_visible_without_exposing_thoughts() {
    let (_dir, hub, id, session) = setup().await;
    let mut events = hub.events.subscribe();
    let s = session.clone();
    let prompt = tokio::spawn(async move { s.prompt("activity".into()).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut thinking = false;
        let mut planning = false;
        loop {
            let state = events.recv().await.unwrap();
            assert!(state.last_activity_at > 0);
            assert!(state.turn_started_at.is_some());
            assert!(!state
                .messages
                .iter()
                .any(|m| m.text.contains("Private fixture reasoning")));
            thinking |= state.activity == "thinking";
            planning |= state.activity == "planning";
            if let Some(tool) = state.messages.iter().find(|m| m.text == "Preparing layout") {
                assert_eq!(state.activity, "tool");
                assert_eq!(tool.status.as_deref(), Some("pending"));
                assert_eq!(
                    state.messages.iter().filter(|m| m.role == "tool").count(),
                    1
                );
                assert!(thinking && planning);
                break;
            }
        }
    })
    .await
    .unwrap();
    session.cancel().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), prompt)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(session.snapshot.lock().await.status, "ready");
    hub.disconnect(&id).await;
}
