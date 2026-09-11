use crate::{
    domain::Project,
    error::{AppError, Result},
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::path::Path;

/// A single connection serializes short read-modify-write transactions.
/// Network introspection never holds a repository transaction open.
pub struct ProjectRepository {
    pool: SqlitePool,
}
impl ProjectRepository {
    pub async fn open(path: &Path) -> Result<Self> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY NOT NULL, document TEXT NOT NULL)").execute(&pool).await?;
        Ok(Self { pool })
    }
    pub async fn list(&self) -> Result<Vec<Project>> {
        let rows: Vec<(String,)> = sqlx::query_as("SELECT document FROM projects")
            .fetch_all(&self.pool)
            .await?;
        let mut projects: Vec<Project> = rows
            .into_iter()
            .map(|(s,)| serde_json::from_str(&s))
            .collect::<std::result::Result<_, _>>()?;
        projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(projects)
    }
    pub async fn get(&self, id: &str) -> Result<Project> {
        let row: Option<(String,)> = sqlx::query_as("SELECT document FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(serde_json::from_str(&row.ok_or(AppError::NotFound)?.0)?)
    }
    pub async fn create(&self, project: Project) -> Result<Project> {
        sqlx::query("INSERT INTO projects (id, document) VALUES (?, ?)")
            .bind(&project.id)
            .bind(serde_json::to_string(&project)?)
            .execute(&self.pool)
            .await?;
        Ok(project)
    }
    pub async fn mutate(
        &self,
        id: &str,
        change: impl FnOnce(&mut Project) -> Result<()>,
    ) -> Result<Project> {
        let mut tx = self.pool.begin().await?;
        let row: Option<(String,)> = sqlx::query_as("SELECT document FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
        let mut project: Project = serde_json::from_str(&row.ok_or(AppError::NotFound)?.0)?;
        change(&mut project)?;
        project.updated_at = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE projects SET document = ? WHERE id = ?")
            .bind(serde_json::to_string(&project)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(project)
    }
    pub async fn delete(&self, id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn persists_and_serializes_concurrent_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("projects.db");
        let repo = ProjectRepository::open(&path).await.unwrap();
        let p = repo
            .create(Project::new("Test".into(), "".into()).unwrap())
            .await
            .unwrap();
        let (a, b) = tokio::join!(
            repo.mutate(&p.id, |p| {
                p.description.push('a');
                Ok(())
            }),
            repo.mutate(&p.id, |p| {
                p.description.push('b');
                Ok(())
            })
        );
        a.unwrap();
        b.unwrap();
        drop(repo);
        let repo = ProjectRepository::open(&path).await.unwrap();
        assert_eq!(repo.get(&p.id).await.unwrap().description.len(), 2);
        assert!(repo
            .mutate(&p.id, |_| Err(AppError::Validation("abort".into())))
            .await
            .is_err());
        assert_eq!(repo.list().await.unwrap().len(), 1);
        repo.delete(&p.id).await.unwrap();
        assert!(repo.get(&p.id).await.is_err());
    }
}
