//! Workspace state: the project list plus the current selection, shared through context.
use crate::api;
use crate::types::{Project, Source};
use leptos::prelude::*;
use std::sync::Arc;

const LAST_PROJECT: &str = "atlas:last-project";

#[derive(Clone, Copy)]
pub struct Workspace {
    pub projects: RwSignal<Vec<Arc<Project>>>,
    pub project_id: RwSignal<Option<String>>,
    pub source_id: RwSignal<Option<String>>,
    pub loading: RwSignal<bool>,
    pub error: RwSignal<String>,
    pub notice: RwSignal<String>,
    pub project: Memo<Option<Arc<Project>>>,
    pub source: Memo<Option<Arc<Source>>>,
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
            project,
            source,
        }
    }

    pub async fn load(self) {
        match api::list_projects().await {
            Ok(list) => {
                self.projects.set(list.into_iter().map(Arc::new).collect());
                let saved = api::local_storage_get(LAST_PROJECT);
                let chosen = self.projects.with_untracked(|list| {
                    list.iter()
                        .find(|p| Some(&p.id) == saved.as_ref())
                        .or(list.first())
                        .map(|p| p.id.clone())
                });
                self.select_project(chosen);
            }
            Err(e) => self.fail(e),
        }
        self.loading.set(false);
    }

    pub fn select_project(self, id: Option<String>) {
        self.project_id.set(id.clone());
        self.source_id.set(self.projects.with_untracked(|list| {
            list.iter()
                .find(|p| Some(&p.id) == id.as_ref())
                .and_then(|p| p.sources.first().map(|s| s.id.clone()))
        }));
        if let Some(id) = id {
            api::local_storage_set(LAST_PROJECT, &id);
        }
    }

    pub fn select_source(self, id: String) {
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
            self.source_id
                .set(project.sources.last().map(|s| s.id.clone()));
        }
        let current = self.source_id.get_untracked();
        if !project
            .sources
            .iter()
            .any(|s| Some(&s.id) == current.as_ref())
        {
            self.source_id
                .set(project.sources.first().map(|s| s.id.clone()));
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
