//! Project-level access to the repository atlas (`.schematlas/` diagrams and docs). The project's
//! working directory is the repository; SQLite never holds diagram or doc content.
use crate::{
    diagram::{CodeRef, Diagram},
    error::{AppError, Result},
    repo::{self, Kind, Listing, Repo, Snippet},
    service::Workspace,
};
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramFile {
    pub slug: String,
    pub hash: String,
    pub diagram: Diagram,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocFile {
    pub slug: String,
    pub hash: String,
    pub title: String,
    pub markdown: String,
}

pub type Watcher = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Run blocking file I/O off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| AppError::Agent("A file operation was interrupted.".into()))?
}

/// Open a repository file in the user's editor: VS Code at the line when it is installed,
/// otherwise the system's default application. The path was checked to be inside the repo.
pub async fn open_in_editor(path: &str, line: Option<u32>) -> Result<()> {
    #[cfg(target_os = "macos")]
    const OPEN: &str = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    const OPEN: &str = "xdg-open";
    #[cfg(windows)]
    const OPEN: &str = "explorer";
    let editor = format!(
        "vscode://file{}{}",
        if path.starts_with('/') { "" } else { "/" },
        path
    ) + &line.map(|l| format!(":{l}")).unwrap_or_default();
    for target in [editor.as_str(), path] {
        let status = tokio::process::Command::new(OPEN)
            .arg(target)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true)
            .status()
            .await;
        if status.is_ok_and(|s| s.success()) {
            return Ok(());
        }
    }
    Err(AppError::Validation(
        "No application could open this file.".into(),
    ))
}

impl Workspace {
    pub async fn repo(&self, project_id: &str) -> Result<Repo> {
        let project = self.repository.get(project_id).await?;
        let directory = project.working_directory.ok_or_else(|| {
            AppError::Validation("Open a repository folder for this project first.".into())
        })?;
        blocking(move || Repo::open(&directory)).await
    }
    pub async fn list_repo(&self, project_id: &str) -> Result<Listing> {
        let project = self.repository.get(project_id).await?;
        let Some(directory) = project.working_directory else {
            return Ok(Listing::default());
        };
        blocking(move || Repo::open(&directory)?.list()).await
    }
    pub async fn get_diagram(&self, project_id: &str, slug: &str) -> Result<DiagramFile> {
        let repo = self.repo(project_id).await?;
        let slug = slug.to_owned();
        blocking(move || {
            let (text, hash) = repo.read(Kind::Diagram, &slug)?.ok_or(AppError::NotFound)?;
            Ok(DiagramFile {
                diagram: Diagram::parse(&text)?,
                slug,
                hash,
            })
        })
        .await
    }
    /// Validate and write a diagram if the file still has the content the caller last read.
    pub async fn save_diagram(
        &self,
        project_id: &str,
        slug: &str,
        mut diagram: Diagram,
        base: Option<String>,
    ) -> Result<DiagramFile> {
        diagram.normalize();
        diagram.validate()?;
        let text = diagram.to_file()?;
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let slug = slug.to_owned();
        let hash = {
            let slug = slug.clone();
            blocking(move || repo.write(Kind::Diagram, &slug, &text, base.as_deref())).await?
        };
        self.atlas_changed(project_id);
        Ok(DiagramFile {
            slug,
            hash,
            diagram,
        })
    }
    pub async fn delete_file(
        &self,
        project_id: &str,
        kind: Kind,
        slug: &str,
        base: String,
    ) -> Result<()> {
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let slug = slug.to_owned();
        blocking(move || repo.delete(kind, &slug, &base)).await?;
        self.atlas_changed(project_id);
        Ok(())
    }
    pub async fn get_doc(&self, project_id: &str, slug: &str) -> Result<DocFile> {
        if slug.ends_with(".md") || slug.ends_with(".markdown") {
            return self.get_toc_doc(project_id, slug).await;
        }
        let repo = self.repo(project_id).await?;
        let slug = slug.to_owned();
        blocking(move || {
            let (markdown, hash) = repo.read(Kind::Doc, &slug)?.ok_or(AppError::NotFound)?;
            Ok(DocFile {
                title: repo::doc_title(&markdown).unwrap_or_else(|| slug.clone()),
                slug,
                hash,
                markdown,
            })
        })
        .await
    }
    pub async fn save_doc(
        &self,
        project_id: &str,
        slug: &str,
        markdown: String,
        base: Option<String>,
    ) -> Result<DocFile> {
        if slug.ends_with(".md") || slug.ends_with(".markdown") {
            return self.save_toc_doc(project_id, slug, markdown, base).await;
        }
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let slug = slug.to_owned();
        let doc = blocking(move || {
            let hash = repo.write(Kind::Doc, &slug, &markdown, base.as_deref())?;
            Ok(DocFile {
                title: repo::doc_title(&markdown).unwrap_or_else(|| slug.clone()),
                slug,
                hash,
                markdown,
            })
        })
        .await?;
        self.atlas_changed(project_id);
        Ok(doc)
    }
    pub async fn code_snippet(&self, project_id: &str, reference: &str) -> Result<Snippet> {
        let repo = self.repo(project_id).await?;
        let code_ref = CodeRef::parse(reference);
        blocking(move || repo.snippet(&code_ref, 6)).await
    }
    pub async fn code_path(&self, project_id: &str, reference: &str) -> Result<String> {
        let repo = self.repo(project_id).await?;
        let code_ref = CodeRef::parse(reference);
        blocking(move || Ok(repo.code_path(&code_ref)?.to_string_lossy().into_owned())).await
    }
    pub fn atlas_changed(&self, project_id: &str) {
        let _ = self.atlas_events.send(project_id.to_owned());
    }
    /// Watch the project's `.schematlas/` folder so edits made by the agent's own file tools,
    /// a text editor or `git pull` reach the open window. One watcher per project.
    pub async fn watch(&self, project_id: &str) -> Result<()> {
        let repo = self.repo(project_id).await?;
        let mut watchers = self.watchers.lock().await;
        if watchers
            .get(project_id)
            .is_some_and(|(root, _)| root == repo.root())
        {
            return Ok(());
        }
        let events = self.atlas_events.clone();
        let id = project_id.to_owned();
        let marker = format!("/{}/", repo::ATLAS_DIR);
        let mut debouncer = new_debouncer(
            Duration::from_millis(250),
            None,
            move |result: DebounceEventResult| {
                let Ok(batch) = result else { return };
                let relevant = batch.iter().any(|event| {
                    event.paths.iter().any(|p| {
                        let p = p.to_string_lossy();
                        p.contains(&marker) && !p.ends_with(".tmp")
                    })
                });
                if relevant {
                    let _ = events.send(id.clone());
                }
            },
        )
        .map_err(|e| AppError::Agent(format!("Cannot watch the repository: {e}")))?;
        // FSEvents watches a whole tree cheaply; events outside `.schematlas/` are dropped above.
        debouncer
            .watch(repo.root(), RecursiveMode::Recursive)
            .map_err(|e| AppError::Agent(format!("Cannot watch the repository: {e}")))?;
        watchers.insert(project_id.to_owned(), (repo.root().to_owned(), debouncer));
        Ok(())
    }
    pub async fn unwatch(&self, project_id: &str) {
        self.watchers.lock().await.remove(project_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Project;
    use serde_json::json;

    #[tokio::test]
    async fn diagrams_live_in_the_repository_not_the_database() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(&temp.path().join("w.sqlite"))
            .await
            .unwrap();
        let project = workspace
            .repository
            .create(Project::new("Repo".into(), "".into()).unwrap())
            .await
            .unwrap();
        assert!(workspace
            .list_repo(&project.id)
            .await
            .unwrap()
            .root
            .is_none());
        let repo_dir = temp.path().join("repo");
        std::fs::create_dir(&repo_dir).unwrap();
        workspace
            .remember_working_directory(&project.id, repo_dir.to_str().unwrap())
            .await
            .unwrap();
        let mut events = workspace.atlas_events.subscribe();
        let diagram: Diagram = serde_json::from_value(json!({
            "title":"Map","nodes":[{"id":"a","label":"A"},{"id":"b","label":"B"}],
            "edges":[{"source":"a","target":"b"}]
        }))
        .unwrap();
        let saved = workspace
            .save_diagram(&project.id, "map", diagram.clone(), None)
            .await
            .unwrap();
        assert_eq!(events.recv().await.unwrap(), project.id);
        assert!(repo_dir.join(".schematlas/diagrams/map.json").is_file());
        let stored = workspace.repository.get(&project.id).await.unwrap();
        assert!(!serde_json::to_string(&stored).unwrap().contains("Map"));
        let loaded = workspace.get_diagram(&project.id, "map").await.unwrap();
        assert_eq!(loaded.hash, saved.hash);
        assert_eq!(loaded.diagram.edges[0].id, "a->b");
        assert!(workspace
            .save_diagram(&project.id, "map", diagram, Some("stale".into()))
            .await
            .is_err());
        let doc = workspace
            .save_doc(&project.id, "intro", "# Intro\nHello".into(), None)
            .await
            .unwrap();
        assert_eq!(doc.title, "Intro");
        let listing = workspace.list_repo(&project.id).await.unwrap();
        assert_eq!((listing.diagrams.len(), listing.docs.len()), (1, 1));
        assert!(!listing.is_git);
    }
}
