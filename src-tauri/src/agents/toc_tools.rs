//! TOC writes share the app's review and compare-and-swap boundary.
use super::{atlas_tools::unified_diff, mcp, session::Session, AgentHub};
use crate::{
    error::{AppError, Result},
    toc_service::{parse, TOC_FILE},
};
use serde_json::{json, Value};

impl AgentHub {
    pub async fn toc_tool(
        &self,
        session: &Session,
        project_id: &str,
        name: &str,
        args: Value,
    ) -> Result<Value> {
        let repo = self.workspace.repo(project_id).await?;
        let current = tokio::task::spawn_blocking(move || repo.read_atlas(TOC_FILE))
            .await
            .map_err(|_| AppError::Validation("TOC loading was interrupted.".into()))??;
        let hash = current.as_ref().map(|(_, hash)| hash.clone());
        let text = current.map(|(text, _)| text).unwrap_or_default();
        if text.len() > 64 * 1024 {
            return Err(AppError::Validation(
                "This TOC exceeds the agent tool's 64 KB limit. Edit it in the repository.".into(),
            ));
        }
        if name == "read_toc" {
            return Ok(json!(format!(
                ".schematlas/toc.yaml\nbase_hash: {}\n{}",
                serde_json::to_string(&hash)?,
                text
            )));
        }
        let args: mcp::WriteTocArgs = serde_json::from_value(args)?;
        if args.yaml.len() > 64 * 1024 {
            return Err(AppError::Validation(
                "Agent TOC proposals are limited to 64 KB.".into(),
            ));
        }
        parse(&args.yaml)?;
        if args.base_hash != hash {
            return Err(AppError::Validation(
                "toc.yaml changed. Read it again before proposing edits.".into(),
            ));
        }
        self.approve(
            session,
            "Documentation structure · toc.yaml".into(),
            "doc",
            json!({"path":".schematlas/toc.yaml", "isNew":hash.is_none(), "diff":unified_diff(&text, &args.yaml, ".schematlas/toc.yaml")}),
        ).await?;
        self.workspace
            .replace_toc(project_id, args.yaml, hash)
            .await?;
        Ok(json!("Saved .schematlas/toc.yaml."))
    }
}
