//! Workspace state: the project list plus the current selection, shared through context.
use crate::api;
use crate::types::{Project, RepoFreshness, RepoListing, Source, SyncSnapshot};
use leptos::prelude::*;
use std::sync::Arc;

/// Whether diagrams also flag code changed only in the working copy.
pub const LOCAL_CHANGES_KEY: &str = "atlas:local-changes";

/// What the user is looking at, told to the agent with each message so "this" has a meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    /// `diagram` or `doc`.
    pub kind: &'static str,
    /// Repository path (or diagram id) of the open file.
    pub path: String,
    /// The selected node of a diagram: (id, label).
    pub node: Option<(String, String)>,
}
impl Focus {
    /// The line appended to a message.
    pub fn context(&self) -> String {
        let mut line = format!("Open in Schematlas: {} `{}`", self.kind, self.path);
        if let Some((id, label)) = &self.node {
            line.push_str(&format!(", selected node `{id}` (\u{201c}{label}\u{201d})"));
        }
        line.push('.');
        line
    }
}

/// What is open from the repository: a diagram by its id (`path` or `path#n`), or a doc by slug.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AtlasItem {
    Diagram(String),
    Doc(String),
}

#[derive(Clone, Copy)]
pub struct Workspace {
    pub projects: RwSignal<Vec<Arc<Project>>>,
    pub project_id: RwSignal<Option<String>>,
    pub source_id: RwSignal<Option<String>>,
    pub loading: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub notice: RwSignal<String>,
    /// Unsent composer text, in memory only and cleared when switching projects.
    pub agent_draft: RwSignal<String>,
    pub agent_starting: RwSignal<bool>,
    pub project: Memo<Option<Arc<Project>>>,
    pub source: Memo<Option<Arc<Source>>>,
    /// The open diagram or doc. Exclusive with `source_id`.
    pub item: RwSignal<Option<AtlasItem>>,
    pub focus: RwSignal<Option<Focus>>,
    /// A node an agent asked to show: (diagram id, node id), taken by that diagram's view.
    pub reveal: RwSignal<Option<(String, String)>>,
    /// Diagrams with nodes whose cited code changed on the reference branch.
    pub stale: RwSignal<RepoFreshness>,
    /// The current project's `.schematlas/` listing.
    pub repo: RwSignal<RepoListing>,
    pub toc: RwSignal<schematlas_ui::toc::Snapshot>,
    /// Team channel state for the current project.
    pub sync: RwSignal<Option<SyncSnapshot>>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self::new()
    }
}

impl Workspace {
    pub fn new() -> Self {
        let projects = RwSignal::new(Vec::<Arc<Project>>::new());
        let project_id = RwSignal::new(None::<String>);
        let source_id = RwSignal::new(None::<String>);
        let project = Memo::new(move |_| {
            let id = project_id.get()?;
            projects.with(|list| list.iter().find(|p| p.id == id).cloned())
        });
        let source = Memo::new(move |_| {
            let id = source_id.get()?;
            project.with(|p| {
                p.as_ref()?
                    .sources
                    .iter()
                    .find(|s| s.id == id)
                    .cloned()
                    .map(Arc::new)
            })
        });
        Self {
            projects,
            project_id,
            source_id,
            loading: RwSignal::new(true),
            error: RwSignal::new(String::new()),
            notice: RwSignal::new(String::new()),
            agent_draft: RwSignal::new(String::new()),
            agent_starting: RwSignal::new(false),
            project,
            source,
            item: RwSignal::new(None),
            focus: RwSignal::new(None),
            reveal: RwSignal::new(None),
            stale: RwSignal::new(RepoFreshness::default()),
            repo: RwSignal::new(RepoListing::default()),
            toc: RwSignal::new(schematlas_ui::toc::Snapshot::default()),
            sync: RwSignal::new(None),
        }
    }

    /// Reload the repository listing; keeps the open item if it still exists.
    pub async fn refresh_repo(self) {
        let Some(id) = self.project_id.get_untracked() else {
            self.repo.set(RepoListing::default());
            return;
        };
        match api::list_repo(&id).await {
            Ok(listing) => {
                if self.project_id.get_untracked().as_deref() != Some(id.as_str()) {
                    return;
                }
                let gone = match self.item.get_untracked() {
                    Some(AtlasItem::Diagram(id)) => !listing.diagrams.iter().any(|d| d.id == id),
                    // Docs opened by path (from the TOC) may live outside the listing.
                    Some(AtlasItem::Doc(slug)) => {
                        !slug.ends_with(".md")
                            && !slug.ends_with(".markdown")
                            && !listing.docs.iter().any(|d| d.slug == slug)
                    }
                    None => false,
                };
                if gone {
                    self.item.set(None);
                }
                self.repo.set(listing);
            }
            Err(e) => {
                self.repo.set(RepoListing::default());
                self.fail(e);
            }
        }
        self.refresh_toc().await;
        self.refresh_stale().await;
    }
    /// Which diagrams cite code that changed (desktop, git repositories only).
    pub async fn refresh_stale(self) {
        let Some(id) = self.project_id.get_untracked() else {
            return;
        };
        if !api::desktop() {
            return;
        }
        let local = api::local_storage_get(LOCAL_CHANGES_KEY).as_deref() == Some("on");
        let found = api::repo_freshness(&id, local).await.unwrap_or_default();
        if self.project_id.get_untracked().as_deref() == Some(id.as_str()) {
            self.stale.set(found);
        }
    }
    pub async fn refresh_toc(self) {
        let Some(id) = self.project_id.get_untracked() else {
            self.toc.set(Default::default());
            return;
        };
        let result = api::get_toc(&id).await;
        if self.project_id.get_untracked().as_deref() != Some(&id) {
            return;
        }
        self.toc
            .set(result.unwrap_or_else(|e| schematlas_ui::toc::Snapshot {
                error: Some(e),
                ..Default::default()
            }));
    }
    pub async fn refresh_sync(self) {
        let Some(id) = self.project_id.get_untracked() else {
            return;
        };
        if !api::desktop() {
            self.sync.set(None);
            return;
        }
        if let Ok(snapshot) = api::sync_status(&id).await {
            if self.project_id.get_untracked().as_deref() == Some(id.as_str()) {
                self.sync.set(Some(snapshot));
            }
        }
    }
    pub fn open_item(self, item: AtlasItem) {
        self.source_id.set(None);
        self.item.set(Some(item));
    }

    pub async fn load(self) {
        match api::list_projects().await {
            Ok(list) => {
                // The app opens on the projects page; one click reopens any of them.
                self.projects.set(list.into_iter().map(Arc::new).collect());
            }
            Err(e) => self.fail(e),
        }
        self.loading.set(false);
    }

    /// Switching projects lands on the repository overview. A project that documents no
    /// repository yet but has data sources opens its first source, as schema projects always did.
    pub fn select_project(self, id: Option<String>) {
        if self.project_id.get_untracked() != id {
            self.agent_draft.set(String::new());
            self.agent_starting.set(false);
        }
        self.project_id.set(id.clone());
        self.source_id.set(self.projects.with_untracked(|list| {
            list.iter()
                .find(|p| Some(&p.id) == id.as_ref())
                .filter(|p| p.working_directory.is_none())
                .and_then(|p| p.sources.first().map(|s| s.id.clone()))
        }));
        self.item.set(None);
        self.toc.set(Default::default());
        self.stale.set(RepoFreshness::default());
        self.sync.set(None);
        leptos::task::spawn_local(async move {
            self.refresh_repo().await;
            self.refresh_sync().await;
        });
    }

    /// Open a repository folder as a project: the project already documenting it, or a new
    /// one named after the folder.
    pub async fn add_repository(self, path: &str) -> Result<(), String> {
        let trimmed = path.trim_end_matches(['/', '\\']);
        let existing = self.projects.with_untracked(|list| {
            list.iter()
                .find(|p| {
                    p.working_directory
                        .as_deref()
                        .is_some_and(|d| d.trim_end_matches(['/', '\\']) == trimmed)
                })
                .map(|p| p.id.clone())
        });
        if let Some(id) = existing {
            self.select_project(Some(id));
            return Ok(());
        }
        let name = crate::components::projects::folder_name(trimmed).to_string();
        let created = api::create_project(&name, "").await?;
        let project = match api::open_repository(&created.id, path).await {
            Ok(project) => project,
            Err(e) => {
                // Keep the empty project visible rather than losing it silently.
                self.upsert(created, false);
                return Err(e);
            }
        };
        self.upsert(project, false);
        Ok(())
    }

    pub fn select_source(self, id: String) {
        self.item.set(None);
        self.source_id.set(Some(id));
    }

    /// Swap in a newer copy of a known project without changing the selection.
    pub fn replace(self, project: Project) {
        let project = Arc::new(project);
        self.projects.update(|list| {
            for slot in list.iter_mut() {
                if slot.id == project.id {
                    *slot = project.clone();
                }
            }
        });
    }

    pub fn upsert(self, project: Project, select_last: bool) {
        let exists = self
            .projects
            .with_untracked(|list| list.iter().any(|p| p.id == project.id));
        let project = Arc::new(project);
        if exists {
            self.replace((*project).clone());
        } else {
            self.projects.update(|list| list.insert(0, project.clone()));
        }
        if self.project_id.get_untracked().as_deref() != Some(project.id.as_str()) {
            self.select_project(Some(project.id.clone()));
        }
        if select_last {
            if let Some(last) = project.sources.last() {
                self.select_source(last.id.clone());
            }
        }
        // A removed source falls back to the overview rather than another source.
        let current = self.source_id.get_untracked();
        if current.is_some()
            && !project
                .sources
                .iter()
                .any(|s| Some(&s.id) == current.as_ref())
        {
            self.source_id.set(None);
        }
    }

    pub fn fail(self, error: impl Into<String>) {
        self.error.set(error.into());
    }

    pub async fn delete_project(self) -> Result<(), String> {
        let Some(id) = self.project_id.get_untracked() else {
            return Ok(());
        };
        api::delete_project(&id).await?;
        self.projects.update(|list| list.retain(|p| p.id != id));
        let next = self
            .projects
            .with_untracked(|list| list.first().map(|p| p.id.clone()));
        self.select_project(next);
        Ok(())
    }
}
