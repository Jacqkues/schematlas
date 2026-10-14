//! Native Codex App Server driver, matching T3 Code's thread/turn lifecycle.
use super::{
    session::{RpcReply, Session},
    types::{bounded, AgentMessage, ReviewOption},
};
use crate::error::{AppError, Result};
use serde_json::{json, Value};
use std::{sync::atomic::Ordering, time::Duration};
use tokio::sync::oneshot;

pub(super) fn choices() -> Vec<ReviewOption> {
    vec![
        ReviewOption {
            option_id: "allow".into(),
            name: "Allow once".into(),
            kind: "allow_once".into(),
        },
        ReviewOption {
            option_id: "reject".into(),
            name: "Reject".into(),
            kind: "reject_once".into(),
        },
    ]
}

impl Session {
    pub(super) async fn codex_initialize(&self) -> Result<()> {
        self.rpc("initialize", json!({"clientInfo":{"name":"schematlas","title":"Schematlas","version":env!("CARGO_PKG_VERSION")}}), Duration::from_secs(30)).await?;
        self.send(json!({"method":"initialized","params":{}}))
            .await?;
        self.loads_sessions.store(true, Ordering::Relaxed);
        let account = self
            .rpc(
                "account/read",
                json!({"refreshToken":false}),
                Duration::from_secs(30),
            )
            .await?;
        let mut state = self.snapshot.lock().await;
        state.agent_name = "Codex".into();
        if account["requiresOpenaiAuth"].as_bool().unwrap_or(true) && account["account"].is_null() {
            state.status = "authentication".into();
            state.auth_methods = vec![json!({"id":"chatgpt","name":"Sign in with ChatGPT"})];
        }
        self.emit(&state);
        Ok(())
    }
    pub(super) async fn codex_authenticate(&self, method: &str) -> Result<()> {
        if method != "chatgpt" || self.snapshot.lock().await.status != "authentication" {
            return Err(AppError::Validation(
                "Unknown authentication method.".into(),
            ));
        }
        let result = self
            .rpc(
                "account/login/start",
                json!({"type":"chatgpt"}),
                Duration::from_secs(30),
            )
            .await?;
        let url = result["authUrl"]
            .as_str()
            .ok_or_else(|| AppError::Agent("Codex did not provide a sign-in URL.".into()))?
            .to_owned();
        tokio::task::spawn_blocking(move || super::discovery::open_browser(&url))
            .await
            .map_err(|_| AppError::Agent("Could not open Codex sign-in.".into()))??;
        tokio::time::timeout(Duration::from_secs(180), async {
            loop {
                let account = self
                    .rpc(
                        "account/read",
                        json!({"refreshToken":false}),
                        Duration::from_secs(10),
                    )
                    .await?;
                if !account["account"].is_null() {
                    let mut state = self.snapshot.lock().await;
                    state.auth_methods.clear();
                    state.error = None;
                    state.status = "connecting".into();
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        })
        .await
        .map_err(|_| {
            AppError::Agent("Sign-in has not completed. Try again when you are ready.".into())
        })?
    }
    pub(super) async fn codex_session(&self, previous: Option<&str>) -> Result<String> {
        if self.snapshot.lock().await.status == "authentication" {
            return Err(AppError::Agent(
                "Sign in to Codex before starting a conversation.".into(),
            ));
        }
        let mut params = json!({"cwd":self.config.cwd,"sandbox":"read-only","approvalPolicy":"on-request","config":{"mcp_servers":{"schema-atlas":self.bridge}}});
        if let Some(id) = previous {
            params["threadId"] = json!(id);
        }
        let result = self
            .rpc(
                if previous.is_some() {
                    "thread/resume"
                } else {
                    "thread/start"
                },
                params,
                Duration::from_secs(60),
            )
            .await?;
        let id = result
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::Agent("Codex did not return a thread ID.".into()))?
            .to_owned();
        let mut state = self.snapshot.lock().await;
        state.session_id = Some(id.clone());
        state.status = "ready".into();
        state.activity = "idle".into();
        state.resumed = previous.is_some();
        state.error = None;
        state.messages.clear();
        if previous.is_some() {
            if let Some(turns) = result.pointer("/thread/turns").and_then(Value::as_array) {
                for turn in turns {
                    if let Some(items) = turn["items"].as_array() {
                        for item in items {
                            let role = match item["type"].as_str() {
                                Some("userMessage") => "user",
                                Some("agentMessage") => "assistant",
                                _ => continue,
                            };
                            let text = item["text"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| text_content(&item["content"]));
                            if !text.is_empty() {
                                state.push(role, text);
                            }
                        }
                    }
                }
            }
        }
        self.emit(&state);
        Ok(id)
    }
    pub(super) async fn codex_prompt(&self, thread: &str, text: &str) -> Result<Value> {
        let rx = self.native_receiver().await;
        let started = self
            .rpc(
                "turn/start",
                json!({"threadId":thread,"input":[{"type":"text","text":text}]}),
                Duration::from_secs(60),
            )
            .await;
        let result = match started {
            Ok(started) => {
                *self.active_turn.lock().await = started
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if self.snapshot.lock().await.status == "cancelling" {
                    self.codex_cancel(thread).await?;
                }
                self.native_wait(rx).await
            }
            Err(e) => {
                self.turn_reply.lock().await.take();
                Err(e)
            }
        };
        self.active_turn.lock().await.take();
        result
    }
    pub(super) async fn codex_cancel(&self, thread: &str) -> Result<()> {
        let turn = self.active_turn.lock().await.clone();
        if let Some(turn) = turn {
            self.rpc(
                "turn/interrupt",
                json!({"threadId":thread,"turnId":turn}),
                Duration::from_secs(10),
            )
            .await?;
        }
        Ok(())
    }
    pub(super) async fn native_receiver(&self) -> oneshot::Receiver<RpcReply> {
        let (tx, rx) = oneshot::channel();
        *self.turn_reply.lock().await = Some(tx);
        rx
    }
    pub(super) async fn native_wait(&self, rx: oneshot::Receiver<RpcReply>) -> Result<Value> {
        match tokio::time::timeout(Duration::from_secs(1200), rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(error))) => Err(AppError::Agent(error)),
            Ok(Err(_)) => Err(AppError::Agent("Agent disconnected.".into())),
            Err(_) => {
                self.stop().await;
                Err(AppError::Agent("Agent turn timed out.".into()))
            }
        }
    }
    pub(super) async fn native_finish(&self, result: RpcReply) {
        if let Some(reply) = self.turn_reply.lock().await.take() {
            let _ = reply.send(result);
        }
    }
    pub(super) async fn native_message(
        &self,
        id: String,
        role: &str,
        text: String,
        status: Option<String>,
        detail: Option<String>,
        append: bool,
    ) {
        let mut state = self.snapshot.lock().await;
        state.activity = if role == "tool" { "tool" } else { "responding" }.into();
        state.last_activity_at = chrono::Utc::now().timestamp_millis();
        if let Some(message) = state.messages.iter_mut().find(|m| m.id == id) {
            if !text.is_empty() {
                message.text = bounded(
                    &if append {
                        format!("{}{text}", message.text)
                    } else {
                        text
                    },
                    48 * 1024,
                );
            }
            if status.is_some() {
                message.status = status;
            }
            if detail.is_some() {
                message.detail = detail.map(|s| bounded(&s, 16 * 1024));
            }
        } else {
            state.messages.push(AgentMessage {
                id,
                role: role.into(),
                text: bounded(&text, 48 * 1024),
                status,
                detail: detail.map(|s| bounded(&s, 16 * 1024)),
            });
        }
        state.trim();
        self.emit(&state);
    }
    pub(super) async fn codex_event(&self, message: &Value) {
        let params = &message["params"];
        if params["threadId"].as_str() != self.snapshot.lock().await.session_id.as_deref() {
            return;
        }
        let method = message["method"].as_str().unwrap_or_default();
        match method {
            "turn/started" => {
                *self.active_turn.lock().await = params
                    .pointer("/turn/id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            "item/agentMessage/delta" => {
                self.native_message(
                    format!(
                        "assistant:{}",
                        params["itemId"].as_str().unwrap_or_default()
                    ),
                    "assistant",
                    params["delta"].as_str().unwrap_or_default().into(),
                    None,
                    None,
                    true,
                )
                .await;
            }
            "item/started" | "item/completed" => {
                let item = &params["item"];
                let kind = item["type"].as_str().unwrap_or_default();
                let id = item["id"].as_str().unwrap_or_default();
                if kind == "agentMessage" {
                    if method == "item/completed" {
                        self.native_message(
                            format!("assistant:{id}"),
                            "assistant",
                            item["text"].as_str().unwrap_or_default().into(),
                            None,
                            None,
                            false,
                        )
                        .await;
                    }
                } else if kind == "reasoning" {
                    let mut state = self.snapshot.lock().await;
                    state.activity = "thinking".into();
                    self.emit(&state);
                } else if matches!(
                    kind,
                    "commandExecution"
                        | "fileChange"
                        | "mcpToolCall"
                        | "webSearch"
                        | "dynamicToolCall"
                ) {
                    let title = item["command"]
                        .as_str()
                        .or_else(|| item["tool"].as_str())
                        .or_else(|| item["query"].as_str())
                        .unwrap_or(kind)
                        .to_owned();
                    let status = if method == "item/started" {
                        "in_progress"
                    } else if item["status"].as_str() == Some("failed") {
                        "failed"
                    } else {
                        "completed"
                    };
                    let detail = item["aggregatedOutput"]
                        .as_str()
                        .map(str::to_owned)
                        .or_else(|| {
                            (!item["result"].is_null()).then(|| item["result"].to_string())
                        });
                    self.native_message(
                        format!("tool:{id}"),
                        "tool",
                        title,
                        Some(status.into()),
                        detail,
                        false,
                    )
                    .await;
                }
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                let mut state = self.snapshot.lock().await;
                state.activity = "thinking".into();
                state.last_activity_at = chrono::Utc::now().timestamp_millis();
                self.emit(&state);
            }
            "turn/plan/updated" => {
                let mut state = self.snapshot.lock().await;
                state.activity = "planning".into();
                self.emit(&state);
            }
            "turn/completed" => {
                let turn = &params["turn"];
                match turn["status"].as_str() {
                    Some("completed") => {
                        self.native_finish(Ok(json!({"stopReason":"end_turn"})))
                            .await
                    }
                    Some("interrupted") => {
                        self.native_finish(Ok(json!({"stopReason":"cancelled"})))
                            .await
                    }
                    _ => {
                        self.native_finish(Err(turn
                            .pointer("/error/message")
                            .and_then(Value::as_str)
                            .unwrap_or("Codex could not complete this turn.")
                            .into()))
                            .await
                    }
                }
            }
            "error" if params["willRetry"].as_bool() != Some(true) => {
                self.native_finish(Err(params
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex reported an error.")
                    .into()))
                    .await;
            }
            _ => {}
        }
    }
    pub(super) async fn codex_request(&self, message: Value) {
        let params = &message["params"];
        let valid = params["threadId"].as_str() == self.snapshot.lock().await.session_id.as_deref()
            && params["threadId"].is_string();
        let method = message["method"].as_str().unwrap_or_default();
        let result = if valid
            && matches!(
                method,
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
            ) {
            let title = params["command"]
                .as_str()
                .unwrap_or(if method.contains("fileChange") {
                    "Apply file changes"
                } else {
                    "Run command"
                });
            let choice = self
                .review(title.into(), "agent".into(), params.clone(), choices())
                .await
                .ok()
                .flatten();
            json!({"decision":if choice.as_deref() == Some("allow") { "accept" } else { "decline" }})
        } else if valid && method == "item/permissions/requestApproval" {
            let choice = self
                .review(
                    "Grant requested permissions".into(),
                    "agent".into(),
                    params.clone(),
                    choices(),
                )
                .await
                .ok()
                .flatten();
            json!({"permissions":if choice.as_deref() == Some("allow") { params["permissions"].clone() } else { json!({}) },"scope":"turn"})
        } else if valid && method == "mcpServer/elicitation/request" {
            let choice = self
                .review(
                    "Agent requests confirmation".into(),
                    "agent".into(),
                    params.clone(),
                    choices(),
                )
                .await
                .ok()
                .flatten();
            json!({"action":if choice.as_deref() == Some("allow") { "accept" } else { "decline" },"content":null})
        } else {
            self.send(json!({"id":message["id"],"error":{"code":-32601,"message":"This client request is not supported."}})).await.ok();
            return;
        };
        self.send(json!({"id":message["id"],"result":result}))
            .await
            .ok();
    }
}

fn text_content(content: &Value) -> String {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
