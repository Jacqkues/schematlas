//! Repository-owned documentation structure; SQLite is read only for legacy migration.
use crate::{
    domain::Project,
    error::{AppError, Result},
    repo::Repo,
    service::Workspace,
    toc::{self, Edit, Entry, Snapshot, Toc},
};

pub const TOC_FILE: &str = toc::TOC_PATH;

#[derive(serde::Serialize)]
pub struct CreatedDoc {
    pub doc: crate::atlas::DocFile,
    pub toc: Snapshot,
}

pub fn parse(text: &str) -> Result<Toc> {
    if text.len() > 1024 * 1024 {
        return Err(AppError::Validation("toc.yaml is limited to 1 MB.".into()));
    }
    let toc: Toc = serde_saphyr::from_str(text)
        .map_err(|e| AppError::Validation(format!("Invalid toc.yaml: {e}")))?;
    toc.validate().map_err(AppError::Validation)?;
    Ok(toc)
}

fn migration(repo: Option<&Repo>, project: &Project) -> Result<Toc> {
    let mut items = vec![];
    if let Some(repo) = repo {
        // Legacy explorer placements only knew `.schematlas/` files.
        for path in repo.discover() {
            let Some(name) = path.strip_prefix(".schematlas/") else {
                continue;
            };
            let Ok(Some((text, _))) = repo.read(&path) else {
                continue;
            };
            let title = crate::repo::doc_title(&text).unwrap_or_else(|| name.to_owned());
            if let Some(slug) = name
                .strip_prefix("docs/")
                .and_then(|n| n.strip_suffix(".md"))
            {
                items.push((
                    format!("doc:{slug}"),
                    "docs".into(),
                    Entry::file(title, path),
                ));
            } else if let Some(slug) = name
                .strip_prefix("diagrams/")
                .and_then(|n| n.rsplit_once('.').map(|(stem, _)| stem))
            {
                let mut entry = Entry::file(title, path.clone());
                entry.kind = Some("diagram".into());
                items.push((format!("diagram:{slug}"), "diagrams".into(), entry));
            }
        }
    }
    items.extend(project.sources.iter().map(|s| {
        (
            format!("source:{}", s.id),
            if s.kind == "database" {
                "databases"
            } else {
                "apis"
            }
            .into(),
            Entry::source(s.name.clone(), s.name.clone(), s.kind.clone()),
        )
    }));
    Ok(toc::migrate(&project.explorer, &items))
}

pub fn load(repo: Option<&Repo>, project: &Project) -> Result<Snapshot> {
    let mut snapshot = Snapshot {
        root: repo.map(|r| r.root().to_string_lossy().into_owned()),
        ..Snapshot::default()
    };
    if let Some((text, hash)) = repo.map(|r| r.read(TOC_FILE)).transpose()?.flatten() {
        snapshot.hash = Some(hash);
        match parse(&text) {
            Ok(toc) => snapshot.toc = toc,
            Err(e) => {
                snapshot.error = Some(e.to_string());
                return Ok(snapshot);
            }
        }
    } else {
        snapshot.toc = migration(repo, project)?;
        if let Err(e) = snapshot.toc.validate() {
            snapshot.error = Some(format!(
                "The existing organization could not be migrated: {e}"
            ));
            return Ok(snapshot);
        }
    }
    for row in snapshot.toc.rows() {
        if let Some(file) = &row.entry.file {
            if let Some(repo) = repo {
                match toc::valid_path(file)
                    .map_err(AppError::Validation)
                    .and_then(|_| repo.read(file))
                {
                    Ok(Some((_, hash))) => {
                        snapshot.hashes.insert(file.clone(), hash);
                    }
                    Ok(None) => {
                        snapshot
                            .issues
                            .insert(row.id, format!("Missing documentation file: {file}"));
                    }
                    Err(e) => {
                        snapshot.issues.insert(row.id, e.to_string());
                    }
                }
            }
        } else if let Some(source) = &row.entry.source {
            let matches = project
                .sources
                .iter()
                .filter(|s| s.name == *source && Some(&s.kind) == row.entry.kind.as_ref())
                .count();
            if matches != 1 {
                snapshot.issues.insert(
                    row.id,
                    format!(
                        "Connect exactly one {} source named “{source}” in this workspace.",
                        row.entry.kind.as_deref().unwrap_or("data")
                    ),
                );
            }
        }
    }
    Ok(snapshot)
}

impl Workspace {
    pub async fn replace_toc(
        &self,
        project_id: &str,
        yaml: String,
        base_hash: Option<String>,
    ) -> Result<String> {
        parse(&yaml)?;
        let _guard = self.atlas_lock.lock().await;
        let repo = self.repo(project_id).await?;
        let hash =
            tokio::task::spawn_blocking(move || repo.write(TOC_FILE, &yaml, base_hash.as_deref()))
                .await
                .map_err(|_| AppError::Validation("TOC saving was interrupted.".into()))??;
        self.atlas_changed(project_id);
        Ok(hash)
    }
    pub async fn create_toc_doc(
        &self,
        project_id: &str,
        title: String,
        parent: Option<String>,
        base_hash: Option<String>,
    ) -> Result<CreatedDoc> {
        let _guard = self.atlas_lock.lock().await;
        let project = self.repository.get(project_id).await?;
        let result = tokio::task::spawn_blocking(move || {
            let repo = Repo::open(
                project
                    .working_directory
                    .as_deref()
                    .ok_or_else(|| AppError::Validation("Open a repository first.".into()))?,
            )?;
            let snapshot = load(Some(&repo), &project)?;
            if let Some(error) = snapshot.error {
                return Err(AppError::Validation(error));
            }
            if snapshot.hash != base_hash {
                return Err(AppError::Validation(
                    "toc.yaml changed since it was read. Reload it and try again.".into(),
                ));
            }
            let mut toc = snapshot.toc;
            let base = toc::document_path(&toc, parent.as_deref(), &title)
                .map_err(AppError::Validation)?;
            let mut path = base.clone();
            let mut suffix = 2;
            while repo.read(&path)?.is_some() {
                path = format!("{}-{suffix}.md", base.trim_end_matches(".md"));
                suffix += 1;
            }
            toc.apply(Edit::Add {
                entry: Entry::file(title.trim().into(), path.clone()),
                parent,
            })
            .map_err(AppError::Validation)?;
            let markdown = format!("# {}\n\n", title.trim());
            let hash = repo.write(&path, &markdown, None)?;
            if let Err(error) = repo.write(TOC_FILE, &toc::to_yaml(&toc), base_hash.as_deref()) {
                if let Err(rollback) = repo.delete(&path, &hash) {
                    return Err(AppError::Validation(format!(
                        "{error} The document remains at {path}: {rollback}"
                    )));
                }
                return Err(error);
            }
            Ok(CreatedDoc {
                doc: crate::atlas::DocFile {
                    slug: path,
                    hash,
                    title: title.trim().into(),
                    markdown,
                },
                toc: load(Some(&repo), &project)?,
            })
        })
        .await
        .map_err(|_| AppError::Validation("Document creation was interrupted.".into()))??;
        self.atlas_changed(project_id);
        Ok(result)
    }
    pub async fn delete_toc_doc(
        &self,
        project_id: &str,
        path: &str,
        base_hash: String,
    ) -> Result<()> {
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || {
            toc::valid_path(&path).map_err(AppError::Validation)?;
            repo.delete(&path, &base_hash)
        })
        .await
        .map_err(|_| AppError::Validation("Document deletion was interrupted.".into()))??;
        self.atlas_changed(project_id);
        Ok(())
    }
    pub async fn get_toc(&self, project_id: &str) -> Result<Snapshot> {
        let project = self.repository.get(project_id).await?;
        tokio::task::spawn_blocking(move || {
            let repo = project
                .working_directory
                .as_deref()
                .map(Repo::open)
                .transpose()?;
            load(repo.as_ref(), &project)
        })
        .await
        .map_err(|_| AppError::Validation("Documentation loading was interrupted.".into()))?
    }
    pub async fn edit_toc(
        &self,
        project_id: &str,
        edit: Option<Edit>,
        base_hash: Option<String>,
    ) -> Result<Snapshot> {
        let _guard = self.atlas_lock.lock().await;
        let project = self.repository.get(project_id).await?;
        let result = tokio::task::spawn_blocking(move || {
            let repo = Repo::open(project.working_directory.as_deref().ok_or_else(|| {
                AppError::Validation(
                    "Open a repository folder before organizing documentation.".into(),
                )
            })?)?;
            let snapshot = load(Some(&repo), &project)?;
            if let Some(error) = snapshot.error {
                return Err(AppError::Validation(error));
            }
            if snapshot.hash != base_hash {
                return Err(AppError::Validation(
                    "toc.yaml changed since it was read. Reload it and try again.".into(),
                ));
            }
            let mut toc = snapshot.toc;
            if let Some(edit) = edit {
                toc.apply(edit).map_err(AppError::Validation)?;
            }
            repo.write(TOC_FILE, &toc::to_yaml(&toc), base_hash.as_deref())?;
            load(Some(&repo), &project)
        })
        .await
        .map_err(|_| AppError::Validation("Documentation saving was interrupted.".into()))??;
        self.atlas_changed(project_id);
        Ok(result)
    }
    pub async fn get_toc_doc(&self, project_id: &str, path: &str) -> Result<crate::atlas::DocFile> {
        let repo = self.repo(project_id).await?;
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || {
            toc::valid_path(&path).map_err(AppError::Validation)?;
            let (markdown, hash) = repo.read(&path)?.ok_or(AppError::NotFound)?;
            Ok(crate::atlas::DocFile {
                title: crate::repo::doc_title(&markdown).unwrap_or_else(|| path.clone()),
                slug: path,
                hash,
                markdown,
            })
        })
        .await
        .map_err(|_| AppError::Validation("Document loading was interrupted.".into()))?
    }
    pub async fn save_toc_doc(
        &self,
        project_id: &str,
        path: &str,
        markdown: String,
        base_hash: Option<String>,
    ) -> Result<crate::atlas::DocFile> {
        if !(path.ends_with(".md") || path.ends_with(".markdown")) {
            return Err(AppError::Validation(
                "Documents need a .md or .markdown file name.".into(),
            ));
        }
        let repo = self.repo(project_id).await?;
        let _guard = self.atlas_lock.lock().await;
        let path = path.to_owned();
        let result = tokio::task::spawn_blocking(move || {
            toc::valid_path(&path).map_err(AppError::Validation)?;
            let hash = repo.write(&path, &markdown, base_hash.as_deref())?;
            Ok(crate::atlas::DocFile {
                title: crate::repo::doc_title(&markdown).unwrap_or_else(|| path.clone()),
                slug: path,
                hash,
                markdown,
            })
        })
        .await
        .map_err(|_| AppError::Validation("Document saving was interrupted.".into()))?;
        self.atlas_changed(project_id);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Repo, Project) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo::open(dir.path().to_str().unwrap()).unwrap();
        let mut project = Project::new("Docs".into(), "".into()).unwrap();
        project.working_directory = Some(dir.path().to_string_lossy().into_owned());
        (dir, repo, project)
    }
    #[test]
    fn yaml_order_round_trip_and_validation() {
        let yaml = "version: 1\nitems:\n  - title: Z first\n    file: start.md\n  - title: A second\n    children:\n      - title: Flow\n        file: flows/checkout.mmd\n";
        let toc = parse(yaml).unwrap();
        assert_eq!(toc.items[0].title, "Z first");
        assert_eq!(parse(&toc::to_yaml(&toc)).unwrap(), toc);
        for bad in [
            "version: 2\nitems: []",
            "version: 1\nitems: [",
            "version: 1\nitems:\n  - title: Outside\n    file: ../x.md",
            "version: 1\nitems:\n  - title: Missing target",
            "version: 1\nitems: []\nsecret: ignored",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn migration_is_read_only_and_bad_yaml_never_falls_back() {
        let (dir, repo, mut project) = fixture();
        repo.write(".schematlas/docs/intro.md", "# Intro", None)
            .unwrap();
        project
            .explorer
            .apply(
                crate::explorer::Edit::Create {
                    id: "custom".into(),
                    name: "Team".into(),
                    parent: None,
                },
                &[("doc:intro".into(), "docs".into())],
            )
            .unwrap();
        project
            .explorer
            .apply(
                crate::explorer::Edit::Move {
                    item: "doc:intro".into(),
                    folder: Some("custom".into()),
                },
                &[("doc:intro".into(), "docs".into())],
            )
            .unwrap();
        let snapshot = load(Some(&repo), &project).unwrap();
        assert!(snapshot.toc.rows().iter().any(|r| r.path == "Team / Intro"));
        assert!(!dir.path().join(".schematlas/toc.yaml").exists());
        let first = repo.write(TOC_FILE, "version: 1\nitems: [", None).unwrap();
        let broken = load(Some(&repo), &project).unwrap();
        assert!(broken.error.is_some());
        assert!(broken.toc.items.is_empty());
        assert_eq!(broken.hash.as_deref(), Some(first.as_str()));
        assert!(repo
            .write(TOC_FILE, "version: 1\nitems: []\n", Some("stale"))
            .is_err());
        assert_eq!(repo.read(TOC_FILE).unwrap().unwrap().1, first);
    }
    #[test]
    fn missing_targets_and_escaping_symlinks_stay_visible() {
        let (dir, repo, project) = fixture();
        // The directory is only inspected by the Unix-only symlink check below.
        let _ = &dir;
        repo.write(
            TOC_FILE,
            "version: 1\nitems:\n  - title: Missing\n    file: missing.md\n",
            None,
        )
        .unwrap();
        let snapshot = load(Some(&repo), &project).unwrap();
        assert_eq!(snapshot.issues.len(), 1);
        assert_eq!(snapshot.toc.items.len(), 1);
        assert!(repo.read("../outside").is_err());
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            std::os::unix::fs::symlink(outside.path(), dir.path().join(".schematlas/link"))
                .unwrap();
            assert!(repo.write(".schematlas/link/doc.md", "# No", None).is_err());
            assert!(repo.read(".schematlas/link/doc.md").is_err());
            assert!(!outside.path().join("doc.md").exists());
        }
    }
    #[tokio::test]
    async fn toc_and_documents_are_portable_and_hash_checked() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("workspace.sqlite");
        let workspace = Workspace::open(&db).await.unwrap();
        let project = workspace
            .repository
            .create(Project::new("Docs".into(), "".into()).unwrap())
            .await
            .unwrap();
        let repo_dir = dir.path().join("repo");
        std::fs::create_dir(&repo_dir).unwrap();
        workspace
            .remember_working_directory(&project.id, repo_dir.to_str().unwrap())
            .await
            .unwrap();
        let first = workspace
            .edit_toc(
                &project.id,
                Some(Edit::Create {
                    title: "Guides".into(),
                    parent: None,
                }),
                None,
            )
            .await
            .unwrap();
        let folder = first.toc.rows()[0].id.clone();
        let doc = workspace
            .create_toc_doc(
                &project.id,
                "Getting started".into(),
                Some(folder),
                first.hash.clone(),
            )
            .await
            .unwrap();
        assert_eq!(doc.doc.slug, ".schematlas/guides/getting-started.md");
        assert!(repo_dir
            .join(".schematlas/guides/getting-started.md")
            .is_file());
        let listed = workspace.list_repo(&project.id).await.unwrap();
        assert_eq!(listed.docs[0].path, ".schematlas/guides/getting-started.md");
        assert_eq!(
            workspace
                .get_doc(&project.id, &listed.docs[0].slug)
                .await
                .unwrap()
                .markdown,
            doc.doc.markdown
        );
        let unchanged = workspace.repository.get(&project.id).await.unwrap();
        assert_eq!(unchanged.explorer, crate::explorer::Explorer::default());
        assert!(workspace
            .edit_toc(
                &project.id,
                Some(Edit::Create {
                    title: "Conflict".into(),
                    parent: None
                }),
                first.hash
            )
            .await
            .is_err());
        let reopened = Workspace::open(&db).await.unwrap();
        assert_eq!(
            reopened.get_toc(&project.id).await.unwrap().toc,
            doc.toc.toc
        );
        // A second workspace sees the same tree and Markdown without any local explorer metadata.
        let other = Workspace::open(&dir.path().join("other.sqlite"))
            .await
            .unwrap();
        let second = other
            .repository
            .create(Project::new("Other".into(), "".into()).unwrap())
            .await
            .unwrap();
        other
            .remember_working_directory(&second.id, repo_dir.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(other.get_toc(&second.id).await.unwrap().toc, doc.toc.toc);
        assert_eq!(
            other
                .get_toc_doc(&second.id, &doc.doc.slug)
                .await
                .unwrap()
                .markdown,
            doc.doc.markdown
        );
        assert!(other
            .save_toc_doc(
                &second.id,
                &doc.doc.slug,
                "stale".into(),
                Some("bad".into())
            )
            .await
            .is_err());
    }
}
