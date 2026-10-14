//! A project keeps routing handles; each harness continues to own the actual history.
use super::{
    discovery,
    session::Session,
    types::{AgentConfig, ConversationSummary, Protocol},
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
        let routes = routes(&project);
        let mut names =
            std::collections::HashMap::<String, std::collections::HashMap<String, String>>::new();
        let mut list = vec![];
        for route in &routes {
            let config = self.conversation_config(&project, route, &agents).await;
            if !names.contains_key(&route.owner) {
                let ids: Vec<_> = routes
                    .iter()
                    .filter(|r| r.owner == route.owner)
                    .map(|r| r.session_id.clone())
                    .collect();
                let titles = if let Some(config) = &config {
                    self.conversation_names(config, current.as_ref(), &ids)
                        .await
                } else {
                    Default::default()
                };
                names.insert(route.owner.clone(), titles);
            }
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
                label: names
                    .get(&route.owner)
                    .and_then(|titles| titles.get(&route.session_id))
                    .cloned()
                    .unwrap_or_else(|| "Untitled chat".into()),
                agent_name: name,
                available: config.is_some(),
            });
        }
        list.reverse();
        Ok(list)
    }
    async fn conversation_names(
        &self,
        config: &AgentConfig,
        current: Option<&std::sync::Arc<Session>>,
        ids: &[String],
    ) -> std::collections::HashMap<String, String> {
        if config.protocol == Protocol::Claude {
            let root = std::env::var_os("CLAUDE_CONFIG_DIR")
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|home| std::path::PathBuf::from(home).join(".claude"))
                });
            let mut names = std::collections::HashMap::new();
            if let Some(root) = root {
                for id in ids {
                    if let Some(title) = super::titles::claude_title(&root, &config.cwd, id).await {
                        names.insert(id.clone(), title);
                    }
                }
            }
            return names;
        }
        if let Some(session) = current.filter(|s| s.config.identity() == config.identity()) {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(8),
                session.conversation_titles(ids),
            )
            .await;
            return session.titles.lock().await.clone();
        }
        // A private metadata-only transport: never create/load a session, attach
        // tools, or emit its connection status into the project's live chat.
        let (events, _) = tokio::sync::broadcast::channel(1);
        let Ok(session) = Session::spawn(
            "metadata".into(),
            config.clone(),
            events,
            &self.endpoint,
            None,
        )
        .await
        else {
            return Default::default();
        };
        let _ = tokio::time::timeout(std::time::Duration::from_secs(8), async {
            session.initialize().await.ok()?;
            Some(session.conversation_titles(ids).await)
        })
        .await;
        let names = session.titles.lock().await.clone();
        session.stop().await;
        names
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
