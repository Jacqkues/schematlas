//! Whether the code a diagram cites has changed since the diagram was checked against it.
//!
//! A node's code references are compared between the commit it was last checked at (its own
//! `commit=` annotation, else the diagram's, else the commit that last touched the diagram file)
//! and the project's reference branch, from `git diff -U0`. Only changes that touch a cited line
//! range count; edits elsewhere in the file just shift the range, which is recomputed. Git is
//! only read: no checkout, fetch or index change.
use schematlas_parser::model::CodeRef;
use serde::Serialize;

/// One `@@ -a,b +c,d @@` hunk of a zero-context diff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    /// The hunk's header and lines, for showing what changed.
    pub text: String,
}

/// The diff of one file between two versions.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDiff {
    pub old_path: String,
    pub new_path: String,
    pub deleted: bool,
    pub hunks: Vec<Hunk>,
}

/// Read `git diff --unified=0 --find-renames` output.
pub fn parse_diff(text: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = vec![];
    let unquote = |p: &str| p.trim().trim_matches('"').to_string();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // `a/old b/new`; paths with spaces are rare and fixed by the rename lines below.
            let (old, new) = rest
                .split_once(" b/")
                .map(|(a, b)| (a.trim_start_matches("a/").to_string(), b.to_string()))
                .unwrap_or_default();
            files.push(FileDiff {
                old_path: unquote(&old),
                new_path: unquote(&new),
                ..FileDiff::default()
            });
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if let Some(path) = line.strip_prefix("rename from ") {
            file.old_path = unquote(path);
        } else if let Some(path) = line.strip_prefix("rename to ") {
            file.new_path = unquote(path);
        } else if line.starts_with("deleted file mode") {
            file.deleted = true;
        } else if let Some(header) = line.strip_prefix("@@ ") {
            let mut parts = header.split_whitespace();
            let range = |spec: Option<&str>, sign: char| -> Option<(u32, u32)> {
                let spec = spec?.strip_prefix(sign)?;
                let (start, len) = spec.split_once(',').unwrap_or((spec, "1"));
                Some((start.parse().ok()?, len.parse().ok()?))
            };
            if let (Some((old_start, old_len)), Some((new_start, new_len))) =
                (range(parts.next(), '-'), range(parts.next(), '+'))
            {
                file.hunks.push(Hunk {
                    old_start,
                    old_len,
                    new_start,
                    new_len,
                    text: line.to_string(),
                });
            }
        } else if line.starts_with('+') || line.starts_with('-') || line.starts_with('\\') {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if let Some(hunk) = file.hunks.last_mut() {
                if hunk.text.len() < 16 * 1024 {
                    hunk.text.push('\n');
                    hunk.text.push_str(line);
                }
            }
        }
    }
    files
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum RefStatus {
    /// Nothing cited changed. `now` is the reference where it sits now (lines may have shifted,
    /// the file may have been renamed).
    Fresh { now: String },
    /// A change touches the cited lines (or the file, for a reference without lines).
    Changed { now: String, patch: String },
    /// The file is gone.
    Broken { reason: String },
}
impl RefStatus {
    pub fn is_fresh(&self) -> bool {
        matches!(self, RefStatus::Fresh { .. })
    }
}

/// How one reference fared in a file's diff (None: the file did not change).
pub fn assess(code_ref: &CodeRef, diff: Option<&FileDiff>) -> RefStatus {
    let Some(diff) = diff else {
        return RefStatus::Fresh {
            now: code_ref.to_string(),
        };
    };
    if diff.deleted {
        return RefStatus::Broken {
            reason: format!("{} was deleted.", code_ref.path),
        };
    }
    let mut now = code_ref.clone();
    now.path = diff.new_path.clone();
    let Some(start) = code_ref.line else {
        return if diff.hunks.is_empty() {
            RefStatus::Fresh {
                now: now.to_string(),
            }
        } else {
            RefStatus::Changed {
                now: now.to_string(),
                patch: patch(&diff.hunks),
            }
        };
    };
    let end = code_ref.end_line.unwrap_or(start).max(start);
    let mut shift: i64 = 0;
    let mut growth: i64 = 0;
    let mut touching = vec![];
    for hunk in &diff.hunks {
        let delta = hunk.new_len as i64 - hunk.old_len as i64;
        // A pure insertion (`-n,0`) goes after old line n.
        let (first, last) = if hunk.old_len == 0 {
            (hunk.old_start + 1, hunk.old_start)
        } else {
            (hunk.old_start, hunk.old_start + hunk.old_len - 1)
        };
        let before = if hunk.old_len == 0 {
            hunk.old_start < start
        } else {
            last < start
        };
        let inside = if hunk.old_len == 0 {
            hunk.old_start >= start && hunk.old_start < end
        } else {
            first <= end && last >= start
        };
        if before {
            shift += delta;
        } else if inside {
            growth += delta;
            touching.push(hunk.clone());
        }
    }
    let moved = |line: u32, by: i64| (line as i64 + by).max(1) as u32;
    now.line = Some(moved(start, shift));
    now.end_line = code_ref
        .end_line
        .map(|e| moved(e.max(start), shift + growth).max(moved(start, shift)));
    if touching.is_empty() {
        RefStatus::Fresh {
            now: now.to_string(),
        }
    } else {
        RefStatus::Changed {
            now: now.to_string(),
            patch: patch(&touching),
        }
    }
}

fn patch(hunks: &[Hunk]) -> String {
    let mut out = String::new();
    for hunk in hunks {
        if out.len() > 6 * 1024 {
            out.push_str("\n…");
            break;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&hunk.text);
    }
    out
}

/// The repository's branches, and which one code references are checked against.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branches {
    pub current: Option<String>,
    pub reference: Option<String>,
    pub all: Vec<String>,
}

/// One cited reference of a node that is no longer as it was checked.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefChange {
    pub reference: String,
    #[serde(flatten)]
    pub status: RefStatus,
    /// Recent commits that touched the file: `abc1234 Subject`.
    pub commits: Vec<String>,
    /// Only changed in the working copy or the checked-out branch, not on the reference branch.
    pub local: bool,
}

/// What may have changed under one node.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeChange {
    /// `changed`, `broken`, or `local` (only in the working copy).
    pub status: String,
    pub baseline: String,
    pub refs: Vec<RefChange>,
}

/// A diagram's nodes whose cited code changed since they were checked.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Freshness {
    /// The reference branch (as resolved, e.g. `origin/main`) and its commit.
    pub branch: String,
    pub commit: String,
    pub nodes: std::collections::BTreeMap<String, NodeChange>,
}

/// How many nodes of one diagram may be out of date.
#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Stale {
    pub changed: usize,
    pub broken: usize,
    pub local: usize,
}

/// Every diagram of the repository with nodes whose cited code changed.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoFreshness {
    pub branch: String,
    pub commit: String,
    pub diagrams: BTreeMap<String, Stale>,
}

use crate::{
    documents::{self, DiagramId},
    error::{AppError, Result},
    service::Workspace,
    sync::git::Git,
};
use std::collections::{BTreeMap, HashMap};

async fn branch_list(git: &Git) -> Result<(Option<String>, Vec<String>)> {
    let current = git
        .query(&["symbolic-ref", "--quiet", "--short", "HEAD"])
        .await
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let all = git
        .query(&[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .await?
        .lines()
        .map(str::trim)
        .filter(|b| !b.is_empty() && !b.ends_with("/HEAD") && !b.contains("schematlas-sync"))
        .map(str::to_owned)
        .collect();
    Ok((current, all))
}

/// The branch to compare against: the project's choice, else `main`, `master`, else HEAD's.
fn pick(chosen: Option<&str>, current: Option<&str>, all: &[String]) -> Option<String> {
    let has = |b: &str| all.iter().any(|x| x == b);
    chosen
        .filter(|b| has(b))
        .map(str::to_owned)
        .or_else(|| {
            ["main", "master"]
                .into_iter()
                .find(|b| has(b))
                .map(str::to_owned)
        })
        .or_else(|| current.map(str::to_owned))
}

async fn commit_of(git: &Git, rev: &str) -> Option<String> {
    if rev.starts_with('-') || rev.contains("..") {
        return None;
    }
    git.query(&[
        "rev-parse",
        "--verify",
        "--quiet",
        &format!("{rev}^{{commit}}"),
    ])
    .await
    .ok()
    .map(|s| s.trim().to_owned())
    .filter(|s| s.len() >= 7)
}

impl Workspace {
    async fn git_for(&self, project_id: &str) -> Result<(Git, crate::domain::Project)> {
        let project = self.repository.get(project_id).await?;
        let repo = self.repo(project_id).await?;
        if !repo.is_git() {
            return Err(AppError::Validation(
                "Code changes are tracked in git repositories.".into(),
            ));
        }
        Ok((Git::reader(repo.root()), project))
    }
    pub async fn branches(&self, project_id: &str) -> Result<Branches> {
        let (git, project) = self.git_for(project_id).await?;
        let (current, all) = branch_list(&git).await?;
        let reference = pick(
            project.reference_branch.as_deref(),
            current.as_deref(),
            &all,
        );
        Ok(Branches {
            current,
            reference,
            all,
        })
    }
    pub async fn set_reference_branch(
        &self,
        project_id: &str,
        branch: &str,
    ) -> Result<crate::domain::Project> {
        let (git, _) = self.git_for(project_id).await?;
        let (_, all) = branch_list(&git).await?;
        if !all.iter().any(|b| b == branch) {
            return Err(AppError::Validation(format!("No branch named `{branch}`.")));
        }
        let branch = branch.to_owned();
        let project = self
            .repository
            .mutate(project_id, move |p| {
                p.reference_branch = Some(branch.clone());
                Ok(())
            })
            .await?;
        self.atlas_changed(project_id);
        Ok(project)
    }

    /// The reference branch as compared: `origin/<branch>` when the remote copy exists.
    async fn reference(
        &self,
        git: &Git,
        project: &crate::domain::Project,
    ) -> Result<(String, String)> {
        let (current, all) = branch_list(git).await?;
        let branch = pick(
            project.reference_branch.as_deref(),
            current.as_deref(),
            &all,
        )
        .ok_or_else(|| AppError::Validation("This repository has no branch yet.".into()))?;
        let remote = format!("origin/{branch}");
        match commit_of(git, &remote).await {
            Some(c) if !branch.contains('/') => Ok((remote, c)),
            _ => {
                let c = commit_of(git, &branch)
                    .await
                    .ok_or_else(|| AppError::Validation(format!("`{branch}` has no commit.")))?;
                Ok((branch, c))
            }
        }
    }

    /// Changed nodes of several diagrams at once: references are grouped by the commit they
    /// were checked at, so the whole set costs one diff per baseline (two with `local`).
    async fn assess_files(
        &self,
        git: &Git,
        files: &[documents::DiagramFile],
        target: &str,
        local: bool,
        commits: bool,
    ) -> BTreeMap<String, BTreeMap<String, NodeChange>> {
        let head = commit_of(git, "HEAD").await;
        // (diagram, node, baseline, reference)
        let mut items: Vec<(String, String, String, CodeRef)> = vec![];
        for file in files {
            if !file
                .diagram
                .nodes
                .iter()
                .any(|n| !n.annotation.code.is_empty())
            {
                continue;
            }
            // A diagram without a recorded check was checked when it was last committed.
            let written = git
                .query(&["log", "-1", "--format=%H", "--", &file.path])
                .await
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty());
            let fallback = written
                .or_else(|| head.clone())
                .unwrap_or_else(|| target.to_owned());
            let diagram_commit = file.diagram.annotation.extra.get("commit").cloned();
            for node in &file.diagram.nodes {
                if node.annotation.code.is_empty() {
                    continue;
                }
                let wanted = node
                    .annotation
                    .extra
                    .get("commit")
                    .or(diagram_commit.as_ref())
                    .cloned();
                let baseline = match wanted {
                    Some(c) => commit_of(git, &c).await.unwrap_or_else(|| fallback.clone()),
                    None => fallback.clone(),
                };
                for code in &node.annotation.code {
                    items.push((
                        file.id.clone(),
                        node.id.clone(),
                        baseline.clone(),
                        code.clone(),
                    ));
                }
            }
        }
        let mut by_baseline: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (_, _, baseline, code) in &items {
            let paths = by_baseline.entry(baseline.clone()).or_default();
            if !paths.contains(&code.path) {
                paths.push(code.path.clone());
            }
        }
        let diff = |range: Vec<String>, paths: Vec<String>| async move {
            let mut args: Vec<String> = [
                "diff",
                "--unified=0",
                "--no-color",
                "--no-ext-diff",
                "--find-renames",
            ]
            .map(str::to_owned)
            .to_vec();
            args.extend(range);
            args.push("--".into());
            args.extend(paths);
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            git.query(&args)
                .await
                .map(|t| parse_diff(&t))
                .unwrap_or_default()
        };
        let mut diffs: HashMap<String, (Vec<FileDiff>, Vec<FileDiff>)> = HashMap::new();
        for (baseline, paths) in by_baseline {
            let shared = if baseline == target {
                vec![]
            } else {
                diff(vec![baseline.clone(), target.to_owned()], paths.clone()).await
            };
            let working = if local {
                diff(vec![baseline.clone()], paths).await
            } else {
                vec![]
            };
            diffs.insert(baseline, (shared, working));
        }
        let find =
            |files: &[FileDiff], path: &str| files.iter().find(|f| f.old_path == path).cloned();
        let mut out: BTreeMap<String, BTreeMap<String, NodeChange>> = BTreeMap::new();
        for (diagram, node, baseline, code) in items {
            let (shared, working) = &diffs[&baseline];
            let on_branch = assess(&code, find(shared, &code.path).as_ref());
            let (status, is_local) = if !on_branch.is_fresh() || !local {
                (on_branch, false)
            } else {
                let here = assess(&code, find(working, &code.path).as_ref());
                let fresh = here.is_fresh();
                (if fresh { on_branch } else { here }, !fresh)
            };
            if status.is_fresh() {
                continue;
            }
            let commits = if commits {
                let range = if is_local {
                    format!("{baseline}..HEAD")
                } else {
                    format!("{baseline}..{target}")
                };
                git.query(&["log", "--format=%h %s", "-n", "5", &range, "--", &code.path])
                    .await
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.chars().take(140).collect())
                    .collect()
            } else {
                vec![]
            };
            let entry = out
                .entry(diagram)
                .or_default()
                .entry(node)
                .or_insert_with(|| NodeChange {
                    status: String::new(),
                    baseline: baseline.clone(),
                    refs: vec![],
                });
            entry.refs.push(RefChange {
                reference: code.to_string(),
                status,
                commits,
                local: is_local,
            });
        }
        for nodes in out.values_mut() {
            for change in nodes.values_mut() {
                change.status = if change
                    .refs
                    .iter()
                    .any(|r| matches!(r.status, RefStatus::Broken { .. }))
                {
                    "broken"
                } else if change.refs.iter().all(|r| r.local) {
                    "local"
                } else {
                    "changed"
                }
                .into();
            }
        }
        out
    }

    /// Nodes of a diagram whose cited code changed on the reference branch since they were
    /// checked; with `local`, also changes only in the working copy.
    pub async fn diagram_freshness(
        &self,
        project_id: &str,
        id: &str,
        local: bool,
    ) -> Result<Freshness> {
        let (git, project) = self.git_for(project_id).await?;
        let repo = self.repo(project_id).await?;
        let diagram_id = DiagramId::parse(id)?;
        let file = tokio::task::spawn_blocking(move || documents::load(&repo, &diagram_id))
            .await
            .map_err(|_| AppError::Validation("Reading the diagram was interrupted.".into()))??;
        let (branch, target) = self.reference(&git, &project).await?;
        let mut found = self
            .assess_files(&git, std::slice::from_ref(&file), &target, local, true)
            .await;
        Ok(Freshness {
            branch,
            commit: target,
            nodes: found.remove(&file.id).unwrap_or_default(),
        })
    }

    /// For the sidebar: how many nodes changed in each diagram of the repository.
    pub async fn repo_freshness(&self, project_id: &str, local: bool) -> Result<RepoFreshness> {
        let (git, project) = self.git_for(project_id).await?;
        let (branch, target) = self.reference(&git, &project).await?;
        let listing = self.list_repo(project_id).await?;
        let repo = self.repo(project_id).await?;
        let ids: Vec<String> = listing
            .diagrams
            .iter()
            .filter(|d| d.error.is_none())
            .map(|d| d.id.clone())
            .collect();
        let files = tokio::task::spawn_blocking(move || {
            ids.iter()
                .filter_map(|id| documents::load(&repo, &DiagramId::parse(id).ok()?).ok())
                .collect::<Vec<_>>()
        })
        .await
        .map_err(|_| AppError::Validation("Reading the diagrams was interrupted.".into()))?;
        let found = self.assess_files(&git, &files, &target, local, false).await;
        Ok(RepoFreshness {
            branch,
            commit: target,
            diagrams: found
                .into_iter()
                .map(|(id, nodes)| {
                    let count = |s: &str| nodes.values().filter(|c| c.status == s).count();
                    (
                        id,
                        Stale {
                            changed: count("changed"),
                            broken: count("broken"),
                            local: count("local"),
                        },
                    )
                })
                .collect(),
        })
    }

    /// What reviewing a node writes: its references moved to where the code is now and the
    /// reference branch's commit recorded as its new check. Returns the diagram as read, the
    /// edit, the commit, and what had changed (None when the node was not flagged).
    pub async fn review_edits(
        &self,
        project_id: &str,
        id: &str,
        node: &str,
    ) -> Result<(
        documents::DiagramFile,
        Vec<schematlas_parser::model::Edit>,
        String,
        Option<NodeChange>,
    )> {
        let freshness = self.diagram_freshness(project_id, id, false).await?;
        let repo = self.repo(project_id).await?;
        let diagram_id = DiagramId::parse(id)?;
        let current = tokio::task::spawn_blocking(move || documents::load(&repo, &diagram_id))
            .await
            .map_err(|_| AppError::Validation("Reading the diagram was interrupted.".into()))??;
        let target = current
            .diagram
            .nodes
            .iter()
            .find(|n| n.id == node)
            .ok_or(AppError::NotFound)?;
        let mut annotation = target.annotation.clone();
        let change = freshness.nodes.get(node).cloned();
        if let Some(change) = &change {
            for code in annotation.code.iter_mut() {
                let reference = code.to_string();
                if let Some(r) = change.refs.iter().find(|r| r.reference == reference) {
                    if let RefStatus::Fresh { now } | RefStatus::Changed { now, .. } = &r.status {
                        *code = CodeRef::parse(now);
                    }
                }
            }
        }
        annotation
            .extra
            .insert("commit".into(), freshness.commit.clone());
        let edits = vec![schematlas_parser::model::Edit::Annotate {
            target: schematlas_parser::annotate::Target::Node(node.to_owned()),
            annotation,
        }];
        Ok((current, edits, freshness.commit, change))
    }

    /// The node's cited code was looked at again: move its references to where the code is now
    /// and record the reference branch's commit as its new check.
    pub async fn review_node(
        &self,
        project_id: &str,
        id: &str,
        node: &str,
        base: String,
    ) -> Result<documents::DiagramFile> {
        let (_, edits, _, _) = self.review_edits(project_id, id, node).await?;
        let repo = self.repo(project_id).await?;
        let diagram_id = DiagramId::parse(id)?;
        let _guard = self.atlas_lock.lock().await;
        let file =
            tokio::task::spawn_blocking(move || documents::edit(&repo, &diagram_id, &edits, &base))
                .await
                .map_err(|_| {
                    AppError::Validation("Saving the diagram was interrupted.".into())
                })??;
        self.atlas_changed(project_id);
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/app/app.py b/app/app.py
index 1..2 100644
--- a/app/app.py
+++ b/app/app.py
@@ -3,0 +4,2 @@ import os
+import sys
+import json
@@ -20,2 +22,3 @@ def create_app():
-    app = FastAPI()
-    return app
+    app = FastAPI(title=\"Omnia\")
+    app.add_middleware(Cors)
+    return app
diff --git a/old/name.py b/new/name.py
similarity index 98%
rename from old/name.py
rename to new/name.py
@@ -1 +1 @@
-x = 1
+x = 2
diff --git a/gone.py b/gone.py
deleted file mode 100644
index 3..0
--- a/gone.py
+++ /dev/null
@@ -1,3 +0,0 @@
-a
-b
-c
";

    fn files() -> Vec<FileDiff> {
        parse_diff(DIFF)
    }
    fn of<'a>(files: &'a [FileDiff], path: &str) -> Option<&'a FileDiff> {
        files.iter().find(|f| f.old_path == path)
    }

    #[test]
    fn diffs_are_read_with_renames_and_deletions() {
        let f = files();
        assert_eq!(f.len(), 3);
        assert_eq!(f[0].hunks.len(), 2);
        assert_eq!((f[0].hunks[0].old_start, f[0].hunks[0].old_len), (3, 0));
        assert_eq!((f[0].hunks[1].new_start, f[0].hunks[1].new_len), (22, 3));
        assert!(f[0].hunks[1].text.contains("+    app.add_middleware(Cors)"));
        assert_eq!(
            (f[1].old_path.as_str(), f[1].new_path.as_str()),
            ("old/name.py", "new/name.py")
        );
        assert!(f[2].deleted);
    }

    #[test]
    fn only_changes_to_cited_lines_count_and_ranges_follow_the_code() {
        let f = files();
        let app = of(&f, "app/app.py");
        // Lines 10-15: two lines were inserted above, nothing inside changed.
        assert_eq!(
            assess(&CodeRef::parse("app/app.py:10-15"), app),
            RefStatus::Fresh {
                now: "app/app.py:12-17".into()
            }
        );
        // Lines 18-21 overlap the rewritten create_app body.
        match assess(&CodeRef::parse("app/app.py:18-21"), app) {
            RefStatus::Changed { now, patch } => {
                assert_eq!(now, "app/app.py:20-24");
                assert!(patch.contains("add_middleware"), "{patch}");
            }
            other => panic!("{other:?}"),
        }
        // A reference without lines sees any change to its file.
        assert!(!assess(&CodeRef::parse("app/app.py"), app).is_fresh());
        // An untouched file stays as it was.
        assert_eq!(
            assess(&CodeRef::parse("app/other.py:5"), None),
            RefStatus::Fresh {
                now: "app/other.py:5".into()
            }
        );
        // Renames carry the reference along; deletions break it.
        assert!(matches!(
            assess(&CodeRef::parse("old/name.py:1"), of(&f, "old/name.py")),
            RefStatus::Changed { now, .. } if now == "new/name.py:1"
        ));
        assert!(matches!(
            assess(&CodeRef::parse("gone.py:2"), of(&f, "gone.py")),
            RefStatus::Broken { .. }
        ));
    }

    fn git(dir: &std::path::Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "T")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "T")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[tokio::test]
    async fn nodes_learn_when_their_code_changed_on_the_reference_branch() {
        if std::process::Command::new("git")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("repo");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join(".schematlas/diagrams")).unwrap();
        git(&dir, &["init", "-q", "-b", "main"]);
        let code = (1..=20).map(|i| format!("line {i}\n")).collect::<String>();
        std::fs::write(dir.join("src/api.rs"), &code).unwrap();
        std::fs::write(
            dir.join(".schematlas/diagrams/flow.mmd"),
            "flowchart LR\n  api[API] --> db[(DB)]\n%% @schematlas node api code=src/api.rs:10-12\n%% @schematlas node db code=src/api.rs:2-3\n",
        )
        .unwrap();
        git(&dir, &["add", "."]);
        git(&dir, &["commit", "-q", "-m", "first"]);
        // On main: one line added at the top (shifts api), and db's lines rewritten.
        let changed =
            code.replace("line 2\n", "line two\n")
                .replacen("line 1\n", "line 1\nline 1b\n", 1);
        std::fs::write(dir.join("src/api.rs"), &changed).unwrap();
        git(&dir, &["commit", "-q", "-am", "rename db lines"]);

        let workspace = Workspace::open(&temp.path().join("w.sqlite"))
            .await
            .unwrap();
        let project = workspace
            .repository
            .create(crate::domain::Project::new("Repo".into(), "".into()).unwrap())
            .await
            .unwrap();
        workspace
            .remember_working_directory(&project.id, dir.to_str().unwrap())
            .await
            .unwrap();
        let branches = workspace.branches(&project.id).await.unwrap();
        assert_eq!(branches.reference.as_deref(), Some("main"));
        let id = ".schematlas/diagrams/flow.mmd";
        let fresh = workspace
            .diagram_freshness(&project.id, id, false)
            .await
            .unwrap();
        assert_eq!(fresh.branch, "main");
        assert_eq!(fresh.nodes.keys().collect::<Vec<_>>(), ["db"], "{fresh:#?}");
        let summary = workspace.repo_freshness(&project.id, false).await.unwrap();
        assert_eq!(
            summary.diagrams.get(id),
            Some(&Stale {
                changed: 1,
                broken: 0,
                local: 0
            })
        );
        let db = &fresh.nodes["db"];
        assert_eq!(db.status, "changed");
        assert!(
            db.refs[0].commits[0].ends_with("rename db lines"),
            "{db:#?}"
        );
        // Reviewing moves the references to where the code is and records the check.
        let file = workspace.get_diagram(&project.id, id).await.unwrap();
        let reviewed = workspace
            .review_node(&project.id, id, "db", file.hash)
            .await
            .unwrap();
        let node = reviewed
            .diagram
            .nodes
            .iter()
            .find(|n| n.id == "db")
            .unwrap();
        // Git reports the added line and the rewritten one as one hunk, so the range widens to
        // cover all of it rather than guess which part it was.
        assert_eq!(node.annotation.code[0].to_string(), "src/api.rs:2-4");
        assert_eq!(node.annotation.extra["commit"], fresh.commit);
        git(&dir, &["commit", "-q", "-am", "review"]);
        let again = workspace
            .diagram_freshness(&project.id, id, false)
            .await
            .unwrap();
        assert!(again.nodes.is_empty(), "{again:#?}");
        // Uncommitted edits show only when asked for, as local.
        let mut text = std::fs::read_to_string(dir.join("src/api.rs")).unwrap();
        text = text.replace("line 11\n", "line eleven\n");
        std::fs::write(dir.join("src/api.rs"), text).unwrap();
        assert!(workspace
            .diagram_freshness(&project.id, id, false)
            .await
            .unwrap()
            .nodes
            .is_empty());
        let local = workspace
            .diagram_freshness(&project.id, id, true)
            .await
            .unwrap();
        assert_eq!(local.nodes["api"].status, "local");
    }

    #[test]
    fn insertions_at_the_edges_of_a_range() {
        let diff = parse_diff(
            "diff --git a/f.rs b/f.rs\n@@ -10,0 +11,1 @@\n+// added after line 10\n@@ -20,0 +22,1 @@\n+// added after line 20\n",
        );
        let f = diff.first();
        // After line 10, inside 10-20: a change in the range.
        assert!(!assess(&CodeRef::parse("f.rs:10-20"), f).is_fresh());
        // After the range's last line: outside it.
        assert!(assess(&CodeRef::parse("f.rs:12-20"), f).is_fresh());
        // Above line 21: shifted by the first insertion only.
        assert_eq!(
            assess(&CodeRef::parse("f.rs:21"), f),
            RefStatus::Fresh {
                now: "f.rs:23".into()
            }
        );
    }
}
