use super::*;
use crate::domain::{ConnectionRequest, DatabaseKind, Project};
use sqlx::{Connection, Row};
fn fixture_agent(dir: &tempfile::TempDir) -> AgentConfig {
    AgentConfig {
        executable: std::env::var("ATLAS_TEST_PYTHON")
            .unwrap_or_else(|_| "/usr/bin/python3".into()),
        args: vec![format!(
            "{}/../examples/mock-acp-agent.py",
            env!("CARGO_MANIFEST_DIR")
        )],
        cwd: dir.path().to_string_lossy().into(),
    }
}
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
    hub.connect(project.id.clone(), fixture_agent(&dir))
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
async fn reconnecting_resumes_the_conversation_the_agent_still_holds() {
    let (dir, hub, id, session) = setup().await;
    // A new project has no conversation yet, so the first connect creates one
    // and records the handle the agent gave back.
    let stored = |hub: Arc<AgentHub>, id: String| async move {
        hub.workspace
            .repository
            .get(&id)
            .await
            .unwrap()
            .agent_session_id
    };
    assert_eq!(
        stored(hub.clone(), id.clone()).await.as_deref(),
        Some("fixture-session")
    );
    assert!(!session.snapshot.lock().await.resumed);

    // Reconnecting replays the agent's own transcript instead of starting over.
    hub.disconnect(&id).await;
    hub.connect(id.clone(), fixture_agent(&dir)).await.unwrap();
    let session = hub.get(&id).await.unwrap();
    let state = session.snapshot.lock().await.clone();
    assert_eq!(state.status, "ready");
    assert!(state.resumed);
    let roles: Vec<&str> = state.messages.iter().map(|m| m.role.as_str()).collect();
    assert_eq!(roles, ["user", "tool", "assistant"]);
    // Consecutive chunks of one replayed message are joined, as they are live.
    assert_eq!(state.messages[0].text, "what tables are there?");
    assert_eq!(state.messages[2].text, "Replayed answer.");
    assert_eq!(state.session_id.as_deref(), Some("fixture-session"));

    // A handle the agent no longer recognises must not strand the panel: the
    // failed load falls back to a new session and the stale handle is dropped.
    hub.disconnect(&id).await;
    hub.workspace
        .repository
        .mutate(&id, |p| {
            p.agent_session_id = Some("forgotten-by-the-agent".into());
            Ok(())
        })
        .await
        .unwrap();
    hub.connect(id.clone(), fixture_agent(&dir)).await.unwrap();
    let session = hub.get(&id).await.unwrap();
    let state = session.snapshot.lock().await.clone();
    assert_eq!(state.status, "ready");
    assert!(!state.resumed);
    assert!(state.messages.is_empty());
    assert_eq!(
        stored(hub.clone(), id.clone()).await.as_deref(),
        Some("fixture-session")
    );

    // Starting a new conversation forgets the handle and empties the panel.
    session.prompt("hello".into()).await.unwrap();
    assert!(!session.snapshot.lock().await.messages.is_empty());
    hub.restart_session(&id).await.unwrap();
    assert!(session.snapshot.lock().await.messages.is_empty());
    assert_eq!(
        stored(hub.clone(), id.clone()).await.as_deref(),
        Some("fixture-session")
    );
    hub.disconnect(&id).await;
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
    // Compact text, and sources and tables are addressable by name.
    let schema = hub
        .tool(
            &session.token,
            "get_schema",
            json!({"source":"Disposable SQLite"}),
        )
        .await
        .unwrap();
    let schema = schema.as_str().unwrap();
    assert!(schema.contains("items"), "{schema}");
    assert!(schema.contains("id INTEGER"), "{schema}");
    assert!(!schema.contains('{'), "results must not be JSON: {schema}");
    assert!(hub
        .tool(
            &session.token,
            "get_schema",
            json!({"source":"outside-project"})
        )
        .await
        .is_err());
    let described = hub
        .tool(
            &session.token,
            "describe_table",
            json!({"source":source_id,"table":"items"}),
        )
        .await
        .unwrap();
    assert!(described.as_str().unwrap().contains("columns:"));
    let found = hub
        .tool(&session.token, "search_schema", json!({"query":"item"}))
        .await
        .unwrap();
    assert!(found.as_str().unwrap().contains("items"));
    // An unknown table names the candidates instead of failing silently.
    let miss = hub
        .tool(
            &session.token,
            "describe_table",
            json!({"source":source_id,"table":"itemz"}),
        )
        .await
        .unwrap_err()
        .to_string();
    assert!(miss.contains("items"), "{miss}");
    for allow in [false, true] {
        let h = hub.clone();
        let token = session.token.clone();
        let args = json!({"source":source_id,"sql":"INSERT INTO items VALUES (7)"});
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
            json!({"source":source_id,"sql":"DELETE FROM items"}),
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
        let args = json!({"source":source.name,"operation":format!("GET {}",operation.name),"path_parameters":{"id":"1"}});
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
    let moved = hub.tool(&session.token,"move_nodes",json!({"source":source_id,"nodes":[{"table":a,"x":120,"y":200},{"table":b,"x":600,"y":200}]})).await.unwrap();
    // Confirmations stay short instead of echoing every saved position.
    assert_eq!(moved.as_str().unwrap(), "Saved. 2 nodes moved.");
    assert_eq!(changes.recv().await.unwrap().id, id);
    assert!(hub.tool(&session.token,"move_nodes",json!({"source":source_id,"nodes":[{"table":a,"x":800,"y":400},{"table":"missing","x":1,"y":2}]})).await.is_err());
    let canvas = hub
        .tool(&session.token, "get_canvas", json!({"source":source_id}))
        .await
        .unwrap();
    let canvas = canvas.as_str().unwrap();
    assert!(canvas.contains("120"), "{canvas}");
    assert!(canvas.contains("nodes:"), "{canvas}");
    let created = hub
        .tool(
            &session.token,
            "create_group",
            json!({"source":source_id,"name":"Customers","tables":[a,b],"color":"#6D9DE3"}),
        )
        .await
        .unwrap();
    assert!(created.as_str().unwrap().contains("2 members"), "{created}");
    let project = hub.workspace.repository.get(&id).await.unwrap();
    assert_eq!(project.sources[0].groups[0].name, "Customers");
    assert_eq!(project.sources[0].groups[0].color, "#6d9de3");
    let group_id = project.sources[0].groups[0].id.clone();
    assert!(hub.tool(&session.token, "create_group", json!({"source":source_id,"group_id":group_id,"name":"Invalid color","tables":[a,b],"color":"red; opacity: 0"})).await.is_err());
    assert_eq!(
        hub.workspace.repository.get(&id).await.unwrap().sources[0].groups[0].name,
        "Customers"
    );
    hub.tool(
        &session.token,
        "create_group",
        json!({"source":source_id,"group_id":group_id,"name":"Customer domain","tables":[a,b]}),
    )
    .await
    .unwrap();
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
            json!({"source":"another-project-source","name":"No","tables":[a]})
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
        json!({"source":source_id,"group_id":group_id}),
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

/// Run a tool that waits for approval, approve it, and return the review plus its result.
async fn allow(
    hub: &Arc<AgentHub>,
    session: &Arc<Session>,
    name: &'static str,
    args: Value,
) -> (types::Review, Result<Value>) {
    let h = hub.clone();
    let token = session.token.clone();
    let call = tokio::spawn(async move { h.tool(&token, name, args).await });
    let request = review(session).await;
    session
        .decide(&request.id, Some("allow".into()))
        .await
        .unwrap();
    (request, call.await.unwrap())
}

#[tokio::test]
async fn inspection_tools_prepare_reviewable_sql_and_render_text() {
    let (dir, hub, id, session) = setup().await;
    let path = dir.path().join("shop.db");
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    for statement in [
        "CREATE TABLE customers(id INTEGER PRIMARY KEY, email TEXT NOT NULL)",
        "CREATE TABLE orders(id INTEGER PRIMARY KEY, customer_id INTEGER NOT NULL REFERENCES customers(id), total NUMERIC)",
        "INSERT INTO customers(email) VALUES ('a@example.com')",
        "INSERT INTO orders(customer_id,total) VALUES (1,42)",
    ] {
        sqlx::query(statement).execute(&mut db).await.unwrap();
    }
    db.close().await.unwrap();
    let project = hub
        .workspace
        .connect(
            &id,
            "Shop",
            ConnectionRequest {
                kind: DatabaseKind::Sqlite,
                connection_string: path.to_string_lossy().into(),
            },
            None,
        )
        .await
        .unwrap();
    let source = project.sources[0].id.clone();

    // Reading the schema never asks for approval.
    let path_text = hub
        .tool(
            &session.token,
            "find_join_path",
            json!({"source":"Shop","from":"orders","to":"customers"}),
        )
        .await
        .unwrap();
    let path_text = path_text.as_str().unwrap();
    assert!(path_text.contains("customer_id"), "{path_text}");
    assert!(path_text.contains("JOIN"), "{path_text}");

    // Prepared statements are shown to the user before anything runs.
    let (request, result) = allow(
        &hub,
        &session,
        "sample_rows",
        json!({"source":source,"table":"orders","limit":5}),
    )
    .await;
    let sql = request.details["sql"].as_str().unwrap().to_owned();
    assert!(sql.to_uppercase().starts_with("SELECT"), "{sql}");
    assert!(sql.contains("orders"), "{sql}");
    let rows = result.unwrap();
    let rows = rows.as_str().unwrap();
    assert!(rows.contains("customer_id"), "{rows}");
    assert!(rows.contains("1 rows"), "{rows}");

    let (request, result) = allow(
        &hub,
        &session,
        "table_stats",
        json!({"source":source,"table":"main.orders"}),
    )
    .await;
    assert!(request.details["note"].is_string());
    assert!(result.unwrap().as_str().unwrap().contains("row_count"));

    let (request, result) = allow(
        &hub,
        &session,
        "explain_sql",
        json!({"source":source,"sql":"SELECT * FROM orders WHERE customer_id = 1"}),
    )
    .await;
    assert!(request.details["sql"]
        .as_str()
        .unwrap()
        .to_uppercase()
        .contains("EXPLAIN"));
    result.unwrap();

    // Writes are rejected by explain, and a non-table target is refused before any review.
    assert!(hub
        .tool(
            &session.token,
            "explain_sql",
            json!({"source":source,"sql":"DELETE FROM orders"})
        )
        .await
        .is_err());
    assert!(hub
        .tool(
            &session.token,
            "sample_rows",
            json!({"source":source,"table":"nope"})
        )
        .await
        .is_err());
    hub.shutdown().await;
}

/// The whole point of the text tool results: a schema an agent can afford to read.
#[tokio::test]
async fn schema_payload_is_far_smaller_than_its_json_form() {
    let (dir, hub, id, session) = setup().await;
    // Copy the fixture so inspection never touches a file in the repository.
    let path = dir.path().join("commerce.sqlite");
    std::fs::copy(
        format!("{}/../examples/commerce.sqlite", env!("CARGO_MANIFEST_DIR")),
        &path,
    )
    .unwrap();
    let project = hub
        .workspace
        .connect(
            &id,
            "Commerce",
            ConnectionRequest {
                kind: DatabaseKind::Sqlite,
                connection_string: path.to_string_lossy().into(),
            },
            None,
        )
        .await
        .unwrap();
    let graph = &project.sources[0].graph;
    assert!(graph.entities.len() >= 6);
    let text = hub
        .tool(&session.token, "get_schema", json!({"source":"Commerce"}))
        .await
        .unwrap();
    let text = text.as_str().unwrap();
    let json = serde_json::to_string(graph).unwrap();
    assert!(
        text.len() * 3 < json.len(),
        "compact schema is {} bytes against {} bytes of JSON",
        text.len(),
        json.len()
    );
    for table in ["orders", "customers", "order_items"] {
        assert!(text.contains(table), "{text}");
    }
    // A canvas edit answers with one line, not with every stored position.
    let saved = hub
        .tool(
            &session.token,
            "move_nodes",
            json!({"source":"Commerce","nodes":[{"table":"main.orders","x":10,"y":20}]}),
        )
        .await
        .unwrap();
    assert!(saved.as_str().unwrap().len() < 40, "{saved}");
    hub.shutdown().await;
}
