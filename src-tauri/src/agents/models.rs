//! Model choices come from the running harness, never a baked-in version list.
use super::{
    session::Session,
    types::{AgentModel, AgentSnapshot, Protocol},
};
use crate::error::{AppError, Result};
use serde_json::{json, Value};
use std::time::Duration;

pub(super) fn catalog(value: &Value, id: &str, name: &str) -> Vec<AgentModel> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .take(200)
        .filter_map(|v| {
            if v["hidden"] == true {
                return None;
            }
            let id = v[id].as_str()?;
            if id.is_empty() || id.len() > 256 {
                return None;
            }
            Some(AgentModel {
                id: id.into(),
                resolved_id: v["resolvedModel"]
                    .as_str()
                    .map(|v| v.chars().take(256).collect()),
                name: v[name].as_str().unwrap_or(id).chars().take(160).collect(),
            })
        })
        .collect()
}
pub(super) fn acp_catalog(state: &mut AgentSnapshot, value: &Value) {
    if let Some(options) = value["configOptions"].as_array() {
        let model = options
            .iter()
            .find(|v| v["category"] == "model" && v["type"] == "select");
        state.models.clear();
        state.model_id = None;
        state.model_config_id = None;
        if let Some(model) = model {
            let flat: Vec<Value> = model["options"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|v| {
                    v["options"]
                        .as_array()
                        .cloned()
                        .unwrap_or_else(|| vec![v.clone()])
                })
                .take(200)
                .collect();
            state.models = catalog(&json!(flat), "value", "name");
            state.model_id = model["currentValue"].as_str().map(str::to_owned);
            state.model_config_id = model["id"].as_str().map(str::to_owned);
        }
    } else if value["models"].is_object() {
        state.models = catalog(&value["models"]["availableModels"], "modelId", "name");
        state.model_id = value["models"]["currentModelId"]
            .as_str()
            .map(str::to_owned);
        state.model_config_id = None;
    }
}
impl Session {
    pub(super) async fn codex_models(&self) {
        let mut models = vec![];
        let mut cursor = Value::Null;
        for _ in 0..10 {
            let Ok(result) = self
                .rpc(
                    "model/list",
                    json!({"cursor":cursor,"limit":100}),
                    Duration::from_secs(15),
                )
                .await
            else {
                break;
            };
            models.extend(catalog(&result["data"], "model", "displayName"));
            cursor = result["nextCursor"].clone();
            if cursor.is_null() || models.len() >= 200 {
                break;
            }
        }
        let mut state = self.snapshot.lock().await;
        state.models = models.into_iter().take(200).collect();
        self.emit(&state);
    }
    pub async fn set_model(&self, model: &str) -> Result<()> {
        let (session, config) = {
            let mut state = self.snapshot.lock().await;
            if state.status != "ready" || !state.reviews.is_empty() {
                return Err(AppError::Agent(
                    "Wait for the current turn to finish before changing models.".into(),
                ));
            }
            if !state.models.iter().any(|v| v.id == model) {
                return Err(AppError::Validation(
                    "Choose a model offered by this agent.".into(),
                ));
            }
            let session = state
                .session_id
                .clone()
                .ok_or_else(|| AppError::Agent("No active session.".into()))?;
            // Reserve the session so a prompt cannot race an in-flight model change.
            state.status = "connecting".into();
            self.emit(&state);
            (session, state.model_config_id.clone())
        };
        let result = match self.config.protocol {
            Protocol::Codex => Ok(Value::Null),
            Protocol::Claude => {
                self.claude_control(json!({"subtype":"set_model","model":model}))
                    .await
            }
            Protocol::Acp => {
                let (method, params) = if let Some(id) = config {
                    (
                        "session/set_config_option",
                        json!({"sessionId":session,"configId":id,"value":model}),
                    )
                } else {
                    (
                        "session/set_model",
                        json!({"sessionId":session,"modelId":model}),
                    )
                };
                self.rpc(method, params, Duration::from_secs(30)).await
            }
        };
        let mut state = self.snapshot.lock().await;
        if state.status == "connecting" {
            state.status = "ready".into();
        }
        if let Ok(value) = &result {
            state.model_id = Some(model.into());
            if self.config.protocol == Protocol::Acp {
                acp_catalog(&mut state, value);
            }
        }
        self.emit(&state);
        result.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn acp_prefers_current_config_options_and_handles_groups_and_updates() {
        let mut state = AgentSnapshot::new("project".into());
        acp_catalog(
            &mut state,
            &json!({"models":{"currentModelId":"legacy","availableModels":[{"modelId":"legacy","name":"Legacy"}]}}),
        );
        assert_eq!(state.model_id.as_deref(), Some("legacy"));
        acp_catalog(
            &mut state,
            &json!({"configOptions":[{"id":"provider-model","category":"model","type":"select","currentValue":"deep","options":[{"group":"recommended","options":[{"value":"fast","name":"Fast"},{"value":"deep","name":"Deep"}]}]}]}),
        );
        assert_eq!(state.models.len(), 2);
        assert_eq!(state.model_id.as_deref(), Some("deep"));
        assert_eq!(state.model_config_id.as_deref(), Some("provider-model"));
        acp_catalog(&mut state, &json!({"configOptions":[]}));
        assert!(state.models.is_empty());
        assert!(state.model_config_id.is_none());
        assert!(state.model_id.is_none());
    }
}
