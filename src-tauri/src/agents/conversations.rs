//! A project keeps routing handles; each harness continues to own the actual history.
use super::{
    discovery,
    types::{AgentConfig, ConversationSummary},
    AgentHub,
};
use crate::{
    domain::{AgentConversation, Project},
    error::{AppError, Result},
};

pub(super) fn routes(project: &Project) -> Vec<AgentConversation> {
    let mut routes = project.agent_conversations.clone();
    if let (Some(id), Some(owner), false) = (
        &project.agent_session_id,
        &project.agent_session_owner,
        project.agent_session_pending,
    ) {
        if !routes
            .iter()
            .any(|r| r.session_id == *id && r.owner == *owner)
        {
            routes.push(AgentConversation {
                session_id: id.clone(),
                owner: owner.clone(),
                agent_id: None,
            });
        }
    }
    routes
}
impl AgentHub {
    async fn conversation_config(
        &self,
        project: &Project,
        route: &AgentConversation,
        agents: &[discovery::InstalledAgent],
    ) -> Option<AgentConfig> {
        if let Ok(session) = self.get(&project.id).await {
            if session.config.identity() == route.owner {
                return Some(session.config.clone());
            }
        }
        let cwd = project.working_directory.as_ref()?;
        agents
            .iter()
            .filter(|a| route.agent_id.as_ref().is_none_or(|id| *id == a.id))
            .cloned()
            .find_map(|a| {
                let config = AgentConfig {
                    executable: a.executable,
                    args: a.args,
                    cwd: cwd.clone(),
                    protocol: a.protocol,
                };
                (config.identity() == route.owner).then_some(config)
            })
    }
    pub async fn conversations(&self, project_id: &str) -> Result<Vec<ConversationSummary>> {
        let project = self.workspace.repository.get(project_id).await?;
        let current = self.get(project_id).await.ok();
        let active = if let Some(current) = &current {
            current.snapshot.lock().await.session_id.clone()
        } else {
            None
        };
        let agents = discovery::discover();
        let mut list = vec![];
        for (index, route) in routes(&project).into_iter().enumerate() {
            let config = self.conversation_config(&project, &route, &agents).await;
            let name = if let Some(config) = &config {
                if let Some(agent) = agents
                    .iter()
                    .find(|a| a.executable == config.executable && a.protocol == config.protocol)
                {
                    agent.name.clone()
                } else if let Some(session) = current
                    .as_ref()
                    .filter(|s| s.config.identity() == route.owner)
                {
                    session.snapshot.lock().await.agent_name.clone()
                } else {
                    "Custom agent".into()
                }
            } else {
                match route.agent_id.as_deref() {
                    Some("codex") => "Codex",
                    Some("claude") => "Claude Code",
                    Some("gemini") => "Gemini CLI",
                    Some("opencode") => "OpenCode",
                    _ => "Agent",
                }
                .into()
            };
            list.push(ConversationSummary {
                active: active.as_ref() == Some(&route.session_id)
                    && current
                        .as_ref()
                        .is_some_and(|s| s.config.identity() == route.owner),
                id: format!("{}:{}", route.owner, route.session_id),
                label: format!("Conversation {}", index + 1),
                agent_name: name,
                available: config.is_some(),
            });
        }
        list.reverse();
        Ok(list)
    }
    pub async fn reopen_conversation(&self, project_id: &str, conversation_id: &str) -> Result<()> {
        let project = self.workspace.repository.get(project_id).await?;
        let route = routes(&project)
            .into_iter()
            .find(|r| format!("{}:{}", r.owner, r.session_id) == conversation_id)
            .ok_or_else(|| {
                AppError::Validation("Conversation not found in this project.".into())
            })?;
        if let Ok(session) = self.get(project_id).await {
            let state = session.snapshot.lock().await;
            if session.config.identity() == route.owner
                && state.session_id.as_deref() == Some(route.session_id.as_str())
                && !matches!(state.status.as_str(), "error" | "disconnected")
            {
                return Ok(());
            }
        }
        let agents = discovery::discover();
        let config = self
            .conversation_config(&project, &route, &agents)
            .await
            .ok_or_else(|| {
                AppError::Agent(
                    "The original agent is unavailable. Reconnect it to reopen this conversation."
                        .into(),
                )
            })?;
        self.connect_conversation(project_id.into(), config, Some(route.session_id.clone()))
            .await
    }
}
