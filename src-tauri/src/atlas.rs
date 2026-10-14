//! Project-level access to the repository's diagrams and docs. The project's working directory is
//! the repository; SQLite never holds diagram or doc content. Diagrams are the repository's own
//! files (Mermaid, draw.io, DBML), docs live in `.schematlas/docs/`.
use crate::{
    documents::{self, DiagramEntry, DiagramFile, DiagramId},
    error::{AppError, Result},
    repo::{self, Format, Repo, Snippet, ATLAS_DIR},
    service::Workspace,
};
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};
use schematlas_parser::model::{CodeRef, Edit};
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocFile {
    pub slug: String,
    pub hash: String,
    pub title: String,
    pub markdown: String,
}
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DocEntry {
    pub slug: String,
    pub path: String,
    pub title: String,
    pub hash: String,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub root: Option<String>,
    pub is_git: bool,
    pub diagrams: Vec<DiagramEntry>,
    pub docs: Vec<DocEntry>,
}

pub type Watcher = Debouncer<RecommendedWatcher, RecommendedCache>;
/// Parsed entries per (project, path), reused while the file's hash is unchanged.
pub type ListingCache = HashMap<(String, String), (String, Vec<DiagramEntry>)>;

/// Run blocking file I/O and parsing off the async runtime.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| AppError::Agent("A file operation was interrupted.".into()))?
}

fn doc_path(slug: &str) -> Result<String> {
    let ok = !slug.is_empty()
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-');
    if !ok {
        return Err(AppError::Validation(format!(
            "`{slug}` is not a valid name. Use lowercase letters, digits and dashes (max 64)."
        )));
    }
    Ok(format!("{ATLAS_DIR}/docs/{slug}.md"))
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

/// The doc id of a Markdown file under `.schematlas/`: the slug of a flat `docs/<slug>.md`
/// (kept for existing links), otherwise its repository path, as the TOC names it.
fn doc_slug(path: &str) -> Option<String> {
    let inside = path.strip_prefix(&format!("{ATLAS_DIR}/"))?;
    if !(inside.ends_with(".md") || inside.ends_with(".markdown")) || inside.starts_with("layout/")
    {
        return None;
    }
    if let Some(slug) = inside
        .strip_prefix("docs/")
        .and_then(|name| name.strip_suffix(".md"))
        .filter(|slug| doc_path(slug).is_ok())
    {
        return Some(slug.to_owned());
    }
    Some(path.to_owned())
}

impl Workspace {
    pub async fn repo(&self, project_id: &str) -> Result<Repo> {
        let project = self.repository.get(project_id).await?;
        let directory = project.working_directory.ok_or_else(|| {
            AppError::Validation("Open a repository folder for this project first.".into())
        })?;
        blocking(move || Repo::open(&directory)).await
    }
    /// Every diagram the repository holds, and the Markdown docs under `.schematlas/`.
    pub async fn list_repo(&self, project_id: &str) -> Result<Listing> {
        let project = self.repository.get(project_id).await?;
        let Some(directory) = project.working_directory else {
            return Ok(Listing::default());
        };
        let cache: ListingCache = self
            .listing_cache
            .lock()
            .await
            .iter()
            .filter(|((p, _), _)| p == project_id)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let id = project_id.to_owned();
        let (listing, fresh) = blocking(move || {
            let repo = Repo::open(&directory)?;
            let mut listing = Listing {
                root: Some(repo.root().to_string_lossy().into_owned()),
                is_git: repo.is_git(),
                ..Listing::default()
            };
            let mut fresh: ListingCache = HashMap::new();
            for path in repo.discover() {
                let Ok(Some((text, hash))) = repo.read(&path) else {
                    continue;
                };
                let key = (id.clone(), path.clone());
                let entries = match cache.get(&key) {
                    Some((h, entries)) if *h == hash => entries.clone(),
                    _ => documents::entries(&path, &text, &hash),
                };
                if let Some(slug) = doc_slug(&path) {
                    listing.docs.push(DocEntry {
                        title: repo::doc_title(&text).unwrap_or_else(|| slug.clone()),
                        slug,
                        path: path.clone(),
                        hash: hash.clone(),
                    });
                }
                listing.diagrams.extend(entries.iter().cloned());
                fresh.insert(key, (hash, entries));
            }
            listing
                .diagrams
                .sort_by_key(|e| (e.title.to_lowercase(), e.id.clone()));
            listing.docs.sort_by_key(|e| e.title.to_lowercase());
            Ok((listing, fresh))
        })
        .await?;
        let mut shared = self.listing_cache.lock().await;
        shared.retain(|(p, _), _| p != project_id);
        shared.extend(fresh);
        Ok(listing)
    }
    pub async fn get_diagram(&self, project_id: &str, id: &str) -> Result<DiagramFile> {
        let repo = self.repo(project_id).await?;
        let id = DiagramId::parse(id)?;
        blocking(move || documents::load(&repo, &id)).await
    }
    /// Apply edits to a diagram file if it still has the content the caller last read.
    pub async fn edit_diagram(
        &self,
        project_id: &str,
        id: &str,
        edits: Vec<Edit>,
        base: String,
    ) -> Result<DiagramFile> {
        let repo = self.repo(project_id).await?;
        let id = DiagramId::parse(id)?;
        let _guard = self.atlas_lock.lock().await;
        let file = blocking(move || documents::edit(&repo, &id, &edits, &base)).await?;
        self.atlas_changed(project_id);
        Ok(file)
    }
    /// Undo: put back an earlier text of the diagram's file.
    pub async fn restore_diagram(
        &self,
        project_id: &str,
        id: &str,
        text: String,
        base: String,
    ) -> Result<DiagramFile> {
        let repo = self.repo(project_id).await?;
        let id = DiagramId::parse(id)?;
        let _guard = self.atlas_lock.lock().await;
        let file = blocking(move || documents::restore(&repo, &id, &text, &base)).await?;
        self.atlas_changed(project_id);
        Ok(file)
    }
    /// A new diagram file; `path` None picks `.schematlas/diagrams/<title>.mmd`.
    pub async fn create_diagram(
        &self,
        project_id: &str,
        path: Option<String>,
        title: &str,
        text: String,
    ) -> Result<DiagramFile> {
        let repo = self.repo(project_id).await?;
        let title = title.to_owned();
        let _guard = self.atlas_lock.lock().await;
        let file = blocking(move || {
            let path = path.unwrap_or_else(|| documents::new_path(&repo, &title));
            documents::create(&repo, &path, &text)
        })
        .await?;
        self.atlas_changed(project_id);
        Ok(file)
    }
    /// Write a connected database's schema to a DBML file in the repository. An existing file
    /// is merged, so notes, groups and annotations written there stay; `base` is the hash the
    /// caller last read of it, or None for a file it expects not to exist yet.
    pub async fn export_dbml(
        &self,
        project_id: &str,
        source_id: &str,
        path: Option<String>,
    ) -> Result<DiagramFile> {
        let project = self.repository.get(project_id).await?;
        let source = project
            .sources
            .iter()
            .find(|s| s.id == source_id)
            .ok_or(AppError::NotFound)?;
        if source.kind != "database" {
            return Err(AppError::Validation(
                "Only database sources export to DBML.".into(),
            ));
        }
        let live = crate::dbml_export::schema(&source.graph);
        let path = path.unwrap_or_else(|| crate::dbml_export::default_path(&source.name));
        if Format::of(&path) != Some(Format::Dbml) {
            return Err(AppError::Validation("Choose a .dbml file.".into()));
        }
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let file = blocking(move || {
            match repo.read(&path)? {
                Some((existing, hash)) => {
                    let merged = schematlas_parser::dbml::merge(&existing, &live)
                        .map_err(|e| AppError::Validation(format!("{path}: {e}")))?;
                    repo.write(&path, &merged, Some(&hash))?;
                }
                None => {
                    repo.write(&path, &schematlas_parser::dbml::write(&live), None)?;
                }
            }
            documents::load(&repo, &DiagramId { path, part: None })
        })
        .await?;
        self.atlas_changed(project_id);
        Ok(file)
    }
    /// Delete a diagram's file. Blocks inside Markdown files are edited, not deleted, here.
    pub async fn delete_diagram(&self, project_id: &str, id: &str, base: String) -> Result<()> {
        let repo = self.repo(project_id).await?;
        let id = DiagramId::parse(id)?;
        if Format::of(&id.path) == Some(Format::Markdown) {
            return Err(AppError::Validation(
                "This diagram is a block inside a Markdown file; edit the file to remove it."
                    .into(),
            ));
        }
        let _guard = self.atlas_lock.lock().await;
        blocking(move || repo.delete(&id.path, &base)).await?;
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
            let (markdown, hash) = repo.read(&doc_path(&slug)?)?.ok_or(AppError::NotFound)?;
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
            let hash = repo.write(&doc_path(&slug)?, &markdown, base.as_deref())?;
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
    pub async fn delete_doc(&self, project_id: &str, slug: &str, base: String) -> Result<()> {
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let slug = slug.to_owned();
        blocking(move || repo.delete(&doc_path(&slug)?, &base)).await?;
        self.atlas_changed(project_id);
        Ok(())
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
    /// Watch the repository so edits made by the agent's own tools, an editor or `git pull`
    /// reach the open window. One watcher per project; only diagram files and `.schematlas/`
    /// count.
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
        let marker = format!("/{ATLAS_DIR}/");
        let mut debouncer = new_debouncer(
            Duration::from_millis(250),
            None,
            move |result: DebounceEventResult| {
                let Ok(batch) = result else { return };
                let relevant = batch.iter().any(|event| {
                    event.paths.iter().any(|p| {
                        let p = p.to_string_lossy();
                        !p.ends_with(".tmp")
                            && !p.contains("/.git/")
                            && !p.contains("/target/")
                            && !p.contains("/node_modules/")
                            && (p.contains(&marker) || Format::of(&p).is_some())
                    })
                });
                if relevant {
                    let _ = events.send(id.clone());
                }
            },
        )
        .map_err(|e| AppError::Agent(format!("Cannot watch the repository: {e}")))?;
        // FSEvents watches a whole tree cheaply; unrelated events are dropped above.
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

    #[tokio::test]
    async fn diagrams_are_the_repositorys_own_files() {
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
        std::fs::create_dir_all(repo_dir.join("docs")).unwrap();
        std::fs::write(repo_dir.join("docs/flow.mmd"), "flowchart LR\n  a --> b\n").unwrap();
        workspace
            .remember_working_directory(&project.id, repo_dir.to_str().unwrap())
            .await
            .unwrap();
        let mut events = workspace.atlas_events.subscribe();
        let created = workspace
            .create_diagram(
                &project.id,
                None,
                "Checkout",
                "flowchart LR\n  api --> db\n".into(),
            )
            .await
            .unwrap();
        assert_eq!(created.path, ".schematlas/diagrams/checkout.mmd");
        assert_eq!(events.recv().await.unwrap(), project.id);
        let edited = workspace
            .edit_diagram(
                &project.id,
                &created.id,
                vec![schematlas_parser::model::Edit::SetLabel {
                    id: "api".into(),
                    label: "API".into(),
                }],
                created.hash.clone(),
            )
            .await
            .unwrap();
        assert!(edited.text.contains("api[API]"));
        // Nothing about diagrams is stored in the workspace database.
        let stored = workspace.repository.get(&project.id).await.unwrap();
        assert!(!serde_json::to_string(&stored)
            .unwrap()
            .contains("flowchart"));
        let listing = workspace.list_repo(&project.id).await.unwrap();
        let ids: Vec<&str> = listing.diagrams.iter().map(|e| e.id.as_str()).collect();
        assert!(
            ids.contains(&"docs/flow.mmd") && ids.contains(&".schematlas/diagrams/checkout.mmd"),
            "{ids:?}"
        );
        // The cache follows file changes.
        std::fs::write(
            repo_dir.join("docs/flow.mmd"),
            "flowchart LR\n  a --> b --> c\n",
        )
        .unwrap();
        let listing = workspace.list_repo(&project.id).await.unwrap();
        assert_eq!(
            listing
                .diagrams
                .iter()
                .find(|e| e.id == "docs/flow.mmd")
                .unwrap()
                .edges,
            2
        );
        workspace
            .delete_diagram(&project.id, &edited.id, edited.hash)
            .await
            .unwrap();
        assert!(!repo_dir.join(".schematlas/diagrams/checkout.mmd").exists());
    }

    #[tokio::test]
    async fn database_sources_export_to_dbml_and_keep_written_notes() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = Workspace::open(&temp.path().join("w.sqlite"))
            .await
            .unwrap();
        let project = workspace.demo().await.unwrap();
        let (db, api) = (&project.sources[0], &project.sources[1]);
        let repo_dir = temp.path().join("repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        workspace
            .remember_working_directory(&project.id, repo_dir.to_str().unwrap())
            .await
            .unwrap();
        assert!(workspace
            .export_dbml(&project.id, &api.id, None)
            .await
            .is_err());
        let file = workspace
            .export_dbml(&project.id, &db.id, None)
            .await
            .unwrap();
        assert_eq!(file.path, ".schematlas/schema/commerce-database.dbml");
        let diagram = &file.diagram;
        assert_eq!(diagram.nodes.len(), db.graph.entities.len());
        assert!(!diagram.edges.is_empty());
        let path = repo_dir.join(&file.path);
        let annotated = format!(
            "{}// @schematlas node main.customers note=\"Who buys\"\n",
            std::fs::read_to_string(&path).unwrap()
        );
        std::fs::write(&path, &annotated).unwrap();
        let again = workspace
            .export_dbml(&project.id, &db.id, None)
            .await
            .unwrap();
        assert_eq!(again.text, annotated);
    }
}
