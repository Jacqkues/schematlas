//! Claude Code's native stream-json control protocol, used by its Agent SDK.
//! Uses the installed CLI and its own account/session storage; no adapter installation.
use super::{codex::choices, session::Session, types::bounded};
use crate::error::{AppError, Result};
use serde_json::{json, Value};
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};
use tokio::{process::Command, sync::oneshot};

impl Session {
    pub(super) async fn claude_control(&self, request: Value) -> Result<Value> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        if let Err(e) = self.send(json!({"type":"control_request","request_id":format!("schematlas-{id}"),"request":request})).await {
            self.pending.lock().await.remove(&id);
            return Err(e);
        }
        let result = tokio::time::timeout(Duration::from_secs(60), rx).await;
        self.pending.lock().await.remove(&id);
        match result {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(error))) => Err(AppError::Agent(error)),
            Ok(Err(_)) => Err(AppError::Agent("Claude Code disconnected.".into())),
            Err(_) => Err(AppError::Agent(
                "Claude Code did not respond. Check its installed version.".into(),
            )),
        }
    }
    pub(super) async fn claude_initialize(&self) -> Result<()> {
        let catalog = self
            .claude_control(json!({"subtype":"initialize","hooks":{}}))
            .await?;
        self.loads_sessions.store(true, Ordering::Relaxed);
        let mut state = self.snapshot.lock().await;
        state.agent_name = "Claude Code".into();
        state.models = super::models::catalog(&catalog["models"], "value", "displayName");
        state.model_id = catalog["model"].as_str().map(str::to_owned);
        self.emit(&state);
        Ok(())
    }
    pub(super) async fn claude_session(&self, resumed: bool) -> Result<String> {
        let id = self
            .native_session
            .clone()
            .ok_or_else(|| AppError::Agent("Claude Code has no session.".into()))?;
        let mut state = self.snapshot.lock().await;
        state.session_id = Some(id.clone());
        state.status = "ready".into();
        state.activity = "idle".into();
        state.resumed = resumed;
        state.error = None;
        self.emit(&state);
        Ok(id)
    }
    pub(super) async fn claude_authenticate(&self, method: &str) -> Result<()> {
        if method != "claude-login" || self.snapshot.lock().await.status != "authentication" {
            return Err(AppError::Validation(
                "Unknown authentication method.".into(),
            ));
        }
        let status = tokio::time::timeout(
            Duration::from_secs(180),
            Command::new(&self.config.executable)
                .args(["auth", "login", "--claudeai"])
                .current_dir(&self.config.cwd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .status(),
        )
        .await
        .map_err(|_| {
            AppError::Agent(
                "Claude sign-in has not completed. Try again when you are ready.".into(),
            )
        })??;
        if !status.success() {
            return Err(AppError::Agent("Claude Code could not sign in.".into()));
        }
        let mut state = self.snapshot.lock().await;
        state.status = "ready".into();
        state.error = None;
        state.auth_methods.clear();
        Ok(())
    }
    pub(super) async fn claude_prompt(&self, session: &str, text: &str) -> Result<Value> {
        let rx = self.native_receiver().await;
        if let Err(e) = self.send(json!({"type":"user","session_id":session,"message":{"role":"user","content":[{"type":"text","text":text}]},"parent_tool_use_id":null})).await {
            self.turn_reply.lock().await.take();
            return Err(e);
        }
        self.native_wait(rx).await
    }
    pub(super) async fn claude_event(self: &Arc<Self>, message: Value) {
        match message["type"].as_str().unwrap_or_default() {
            "control_response" => {
                let response = &message["response"];
                let id = response["request_id"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("schematlas-"))
                    .and_then(|s| s.parse::<u64>().ok());
                if let Some(id) = id {
                    if let Some(reply) = self.pending.lock().await.remove(&id) {
                        let result = if response["subtype"].as_str() == Some("error") {
                            Err(response["error"]
                                .as_str()
                                .unwrap_or("Claude control request failed.")
                                .into())
                        } else {
                            Ok(response["response"].clone())
                        };
                        let _ = reply.send(result);
                    }
                }
            }
            "control_request" => {
                let session = self.clone();
                tokio::spawn(async move {
                    session.claude_request(message).await;
                });
            }
            "control_cancel_request" => self.cancel_reviews().await,
            "stream_event" => {
                let event = &message["event"];
                match event["type"].as_str().unwrap_or_default() {
                    "message_start" => {
                        *self.active_turn.lock().await = event
                            .pointer("/message/id")
                            .and_then(Value::as_str)
                            .map(str::to_owned);
                    }
                    "content_block_delta" => {
                        if let Some(text) = event.pointer("/delta/text").and_then(Value::as_str) {
                            let id = self
                                .active_turn
                                .lock()
                                .await
                                .clone()
                                .unwrap_or_else(|| "current".into());
                            self.native_message(
                                format!("assistant:{id}"),
                                "assistant",
                                text.into(),
                                None,
                                None,
                                true,
                            )
                            .await;
                        } else if event.pointer("/delta/type").and_then(Value::as_str)
                            == Some("thinking_delta")
                        {
                            let mut state = self.snapshot.lock().await;
                            state.activity = "thinking".into();
                            state.last_activity_at = chrono::Utc::now().timestamp_millis();
                            self.emit(&state);
                        }
                    }
                    _ => {}
                }
            }
            "assistant" => {
                let content = &message["message"]["content"];
                let id = message
                    .pointer("/message/id")
                    .and_then(Value::as_str)
                    .unwrap_or("current");
                let text = content
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    self.native_message(
                        format!("assistant:{id}"),
                        "assistant",
                        text,
                        None,
                        None,
                        false,
                    )
                    .await;
                }
                for block in content.as_array().into_iter().flatten() {
                    if block["type"] == "tool_use" {
                        self.native_message(
                            format!("tool:{}", block["id"].as_str().unwrap_or("unknown")),
                            "tool",
                            block["name"].as_str().unwrap_or("Agent tool").into(),
                            Some("in_progress".into()),
                            Some(block["input"].to_string()),
                            false,
                        )
                        .await;
                    }
                }
            }
            "user" => {
                for block in message
                    .pointer("/message/content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if block["type"] == "tool_result" {
                        let detail = block["content"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| block["content"].to_string());
                        self.native_message(
                            format!(
                                "tool:{}",
                                block["tool_use_id"].as_str().unwrap_or("unknown")
                            ),
                            "tool",
                            String::new(),
                            Some(
                                if block["is_error"] == true {
                                    "failed"
                                } else {
                                    "completed"
                                }
                                .into(),
                            ),
                            Some(detail),
                            false,
                        )
                        .await;
                    }
                }
            }
            "system" if message["subtype"] == "init" => {
                let mut state = self.snapshot.lock().await;
                state.model_id = message["model"]
                    .as_str()
                    .map(str::to_owned)
                    .or(state.model_id.take());
                self.emit(&state);
            }
            "system" if message["subtype"] == "compact_boundary" => {
                let mut state = self.snapshot.lock().await;
                state.activity = "compacting".into();
                self.emit(&state);
            }
            "result" => {
                let subtype = message["subtype"].as_str().unwrap_or_default();
                if message["is_error"] == true || subtype.starts_with("error_") {
                    let error = message["errors"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .filter(|s| !s.is_empty())
                        .or_else(|| message["result"].as_str().map(str::to_owned))
                        .unwrap_or_else(|| "Claude could not complete this turn.".into());
                    let lower = error.to_lowercase();
                    if lower.contains("not logged in")
                        || lower.contains("login required")
                        || lower.contains("authentication")
                    {
                        let mut state = self.snapshot.lock().await;
                        state.status = "authentication".into();
                        state.auth_methods =
                            vec![json!({"id":"claude-login","name":"Sign in to Claude"})];
                        self.emit(&state);
                    }
                    self.native_finish(Err(bounded(&error, 4000))).await;
                } else {
                    self.native_finish(Ok(json!({"stopReason":"end_turn"})))
                        .await;
                }
            }
            _ => {}
        }
    }
    async fn claude_request(&self, message: Value) {
        let request = &message["request"];
        let result = if request["subtype"] == "can_use_tool" {
            let name = request["tool_name"].as_str().unwrap_or_default();
            // Our MCP server performs its own project-scoped review. This only permits
            // the CLI to reach that server; it does not authorize SQL, HTTP or writes.
            let allow = if name.starts_with("mcp__schema-atlas__") {
                true
            } else {
                self.review(
                    name.into(),
                    "agent".into(),
                    request["input"].clone(),
                    choices(),
                )
                .await
                .ok()
                .flatten()
                .as_deref()
                    == Some("allow")
            };
            if allow {
                json!({"behavior":"allow","updatedInput":request["input"]})
            } else {
                json!({"behavior":"deny","message":"The user declined this request."})
            }
        } else {
            self.send(json!({"type":"control_response","response":{"subtype":"error","request_id":message["request_id"],"error":"This control request is not supported."}})).await.ok();
            return;
        };
        self.send(json!({"type":"control_response","response":{"subtype":"success","request_id":message["request_id"],"response":result}})).await.ok();
    }
}
