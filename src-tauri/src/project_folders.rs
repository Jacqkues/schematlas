//! Project-owned working folders, independent of database connections and agent processes.
use crate::{
    error::{AppError, Result},
    service::Workspace,
};
use std::{io::ErrorKind, path::Path};

fn folder_name(name: &str) -> String {
    let mut name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '-'
            }
        })
        .collect();
    // Leave room for a collision suffix within filesystem component limits.
    while name.len() > 180 {
        name.pop();
    }
    let name = name.trim_matches([' ', '-', '_']);
    if name.is_empty() {
        "Project".into()
    } else {
        name.into()
    }
}

impl Workspace {
    /// Lazily create a project-named folder. Persist the path so renaming the project
    /// or restarting the app never silently moves or replaces a user's repository.
    pub async fn working_directory(&self, project_id: &str) -> Result<String> {
        let _guard = self.folders_lock.lock().await;
        let project = self.repository.get(project_id).await?;
        if let Some(path) = project.working_directory {
            return Ok(path);
        }
        tokio::fs::create_dir_all(&self.folders_root).await?;
        let name = folder_name(&project.name);
        let mut path = self.folders_root.join(&name);
        loop {
            match tokio::fs::create_dir(&path).await {
                Ok(()) => break,
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    // Never claim a pre-existing folder, including another project's
                    // same-name folder, a symlink, or an orphan from a previous run.
                    path = self
                        .folders_root
                        .join(format!("{name}-{}", uuid::Uuid::new_v4()));
                }
                Err(e) => return Err(e.into()),
            }
        }
        let directory = path.to_string_lossy().into_owned();
        self.repository
            .mutate(project_id, |p| {
                p.working_directory = Some(directory.clone());
                Ok(())
            })
            .await?;
        Ok(directory)
    }

    /// Remember a user-selected existing repository. Never create it or initialize Git.
    pub async fn remember_working_directory(
        &self,
        project_id: &str,
        directory: &str,
    ) -> Result<String> {
        let _guard = self.folders_lock.lock().await;
        self.repository.get(project_id).await?;
        let path = Path::new(directory);
        if !path.is_absolute() || !tokio::fs::metadata(path).await.is_ok_and(|m| m.is_dir()) {
            return Err(AppError::Validation(
                "Choose an existing absolute working directory.".into(),
            ));
        }
        let directory = tokio::fs::canonicalize(path)
            .await?
            .to_string_lossy()
            .into_owned();
        self.repository
            .mutate(project_id, |p| {
                p.working_directory = Some(directory.clone());
                Ok(())
            })
            .await?;
        Ok(directory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Project;

    #[tokio::test]
    async fn project_folders_are_distinct_stable_and_remember_repository_choices() {
        let temp = tempfile::tempdir().unwrap();
        let db = temp.path().join("workspace.sqlite");
        let workspace = Workspace::open(&db).await.unwrap();
        let a = workspace
            .repository
            .create(Project::new("Commerce".into(), "".into()).unwrap())
            .await
            .unwrap();
        let b = workspace
            .repository
            .create(Project::new("Commerce".into(), "".into()).unwrap())
            .await
            .unwrap();
        let (first, same) = tokio::join!(
            workspace.working_directory(&a.id),
            workspace.working_directory(&a.id)
        );
        let first = first.unwrap();
        assert_eq!(same.unwrap(), first);
        assert_eq!(Path::new(&first).file_name().unwrap(), "Commerce");
        let second = workspace.working_directory(&b.id).await.unwrap();
        assert_ne!(first, second);
        assert!(Path::new(&second).is_dir());
        workspace
            .repository
            .mutate(&a.id, |p| {
                p.name = "Renamed".into();
                Ok(())
            })
            .await
            .unwrap();
        assert_eq!(workspace.working_directory(&a.id).await.unwrap(), first);
        let repo = temp.path().join("existing-repository");
        tokio::fs::create_dir(&repo).await.unwrap();
        tokio::fs::write(repo.join("keep.txt"), "existing files")
            .await
            .unwrap();
        workspace
            .remember_working_directory(&a.id, repo.to_str().unwrap())
            .await
            .unwrap();
        let reopened = Workspace::open(&db).await.unwrap();
        assert_eq!(
            reopened.working_directory(&a.id).await.unwrap(),
            tokio::fs::canonicalize(&repo)
                .await
                .unwrap()
                .to_string_lossy()
        );
        assert!(workspace
            .remember_working_directory(&a.id, "relative/path")
            .await
            .is_err());
        assert!(workspace
            .remember_working_directory(&a.id, repo.join("keep.txt").to_str().unwrap())
            .await
            .is_err());
        workspace.delete_project(&a.id).await.unwrap();
        assert_eq!(
            tokio::fs::read_to_string(repo.join("keep.txt"))
                .await
                .unwrap(),
            "existing files"
        );
    }

    #[test]
    fn folder_names_cannot_escape_the_workspace() {
        assert_eq!(folder_name("../../Customers/Équipe"), "Customers-Équipe");
        assert_eq!(folder_name("..."), "Project");
        assert_eq!(folder_name("My project"), "My project");
        assert!(folder_name(&"界".repeat(80)).len() <= 180);
        let project: Project = serde_json::from_value(serde_json::json!({
            "id":"legacy", "name":"Old", "description":"", "createdAt":"", "updatedAt":"", "sources":[]
        })).unwrap();
        assert!(project.working_directory.is_none());
    }
}
