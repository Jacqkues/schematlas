//! The git CLI, used with plumbing only: the team branch is read and written without touching the
//! user's HEAD, index or working tree, and the user's own credential helper or SSH agent does the
//! authentication. Prompts are disabled so a missing credential fails fast instead of hanging.
use crate::error::{AppError, Result};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncWriteExt;

pub const BRANCH: &str = "schematlas-sync";
const REMOTE_REF: &str = "refs/heads/schematlas-sync";
/// Fetched copy of the remote branch, outside `refs/heads` and `refs/remotes` so it never shows
/// up as a local branch or in `git branch -a`.
pub const LOCAL_REF: &str = "refs/schematlas/remote";
const TIMEOUT: Duration = Duration::from_secs(30);

pub struct Git {
    repo: PathBuf,
    index: PathBuf,
}

pub enum Push {
    Pushed,
    /// Someone else moved the branch first; fetch, rebuild and retry.
    Stale,
}

/// Remove `user:password@` from a URL before it is stored or shown.
pub fn redact(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) if !parsed.username().is_empty() || parsed.password().is_some() => {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.to_string()
        }
        _ => url.to_owned(),
    }
}

impl Git {
    pub async fn open(repo: &Path) -> Result<Self> {
        let probe = Git {
            repo: repo.to_owned(),
            index: PathBuf::new(),
        };
        let git_dir = probe
            .run(&["rev-parse", "--absolute-git-dir"], None, &[])
            .await
            .map_err(|_| {
                AppError::Validation("Team sync needs a git repository with git installed.".into())
            })?;
        let state = PathBuf::from(git_dir.trim()).join("schematlas");
        tokio::fs::create_dir_all(&state).await?;
        Ok(Git {
            repo: repo.to_owned(),
            index: state.join("index"),
        })
    }
    pub fn state_dir(&self) -> &Path {
        self.index.parent().unwrap_or(&self.repo)
    }
    async fn run(
        &self,
        args: &[&str],
        stdin: Option<&[u8]>,
        env: &[(&str, &Path)],
    ) -> Result<String> {
        let mut command = tokio::process::Command::new("git");
        command
            .arg("-C")
            .arg(&self.repo)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .env("GCM_INTERACTIVE", "never")
            .env("LC_ALL", "C")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        for (key, value) in env {
            command.env(key, value);
        }
        let mut child = command
            .spawn()
            .map_err(|_| AppError::Validation("git is not installed or not on PATH.".into()))?;
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            pipe.write_all(input).await?;
            drop(pipe);
        }
        let output = tokio::time::timeout(TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| AppError::Validation("git did not answer within 30 seconds.".into()))??;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.status.success() {
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(AppError::Validation(format!(
                "git {}: {}",
                args.first().unwrap_or(&""),
                redact_text(stderr.trim())
                    .chars()
                    .take(400)
                    .collect::<String>()
            )))
        }
    }
    pub async fn config(&self, key: &str) -> Option<String> {
        self.run(&["config", "--get", key], None, &[])
            .await
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty())
    }
    pub async fn remotes(&self) -> Result<Vec<String>> {
        Ok(self
            .run(&["remote"], None, &[])
            .await?
            .lines()
            .map(str::to_owned)
            .collect())
    }
    pub async fn remote_url(&self, remote: &str) -> Result<String> {
        valid_remote(remote)?;
        Ok(redact(
            self.run(&["remote", "get-url", remote], None, &[])
                .await?
                .trim(),
        ))
    }
    /// The remote branch tip, or None when the branch does not exist there.
    pub async fn remote_tip(&self, remote: &str) -> Result<Option<String>> {
        valid_remote(remote)?;
        let out = self
            .run(&["ls-remote", "--heads", remote, REMOTE_REF], None, &[])
            .await?;
        Ok(out
            .lines()
            .find(|l| l.ends_with(REMOTE_REF))
            .and_then(|l| l.split_whitespace().next())
            .map(str::to_owned))
    }
    pub async fn fetch(&self, remote: &str) -> Result<()> {
        valid_remote(remote)?;
        let refspec = format!("+{REMOTE_REF}:{LOCAL_REF}");
        self.run(
            &[
                "fetch",
                "--quiet",
                "--no-tags",
                "--no-write-fetch-head",
                remote,
                &refspec,
            ],
            None,
            &[],
        )
        .await
        .map(|_| ())
    }
    /// Every file of a commit's tree as (path, text).
    pub async fn read_tree(&self, commit: &str) -> Result<Vec<(String, String)>> {
        let listing = self
            .run(&["ls-tree", "-r", "-z", "--full-tree", commit], None, &[])
            .await?;
        let mut paths = vec![];
        let mut shas = String::new();
        for entry in listing.split('\0').filter(|e| !e.is_empty()) {
            let Some((meta, path)) = entry.split_once('\t') else {
                continue;
            };
            let mut parts = meta.split_whitespace();
            if parts.next() != Some("100644") || parts.next() != Some("blob") {
                continue;
            }
            let Some(sha) = parts.next() else { continue };
            paths.push(path.to_owned());
            shas.push_str(sha);
            shas.push('\n');
        }
        if paths.is_empty() {
            return Ok(vec![]);
        }
        let batch = self.batch(&shas).await?;
        Ok(paths.into_iter().zip(batch).collect())
    }
    async fn batch(&self, shas: &str) -> Result<Vec<String>> {
        let mut command = tokio::process::Command::new("git");
        command
            .arg("-C")
            .arg(&self.repo)
            .args(["cat-file", "--batch"])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        // Write concurrently: cat-file answers as it reads, so a full stdout pipe would block it.
        let pipe = child.stdin.take();
        let input = shas.to_owned();
        let writer = tokio::spawn(async move {
            if let Some(mut pipe) = pipe {
                let _ = pipe.write_all(input.as_bytes()).await;
            }
        });
        let output = tokio::time::timeout(TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| AppError::Validation("git did not answer within 30 seconds.".into()))??;
        let _ = writer.await;
        let bytes = output.stdout;
        let mut blobs = vec![];
        let mut pos = 0;
        while pos < bytes.len() {
            let end = bytes[pos..]
                .iter()
                .position(|b| *b == b'\n')
                .map(|i| pos + i)
                .ok_or_else(|| AppError::Validation("Unreadable sync branch.".into()))?;
            let header = String::from_utf8_lossy(&bytes[pos..end]).into_owned();
            let size: usize = header
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| AppError::Validation("Unreadable sync branch.".into()))?;
            let start = end + 1;
            let stop = (start + size).min(bytes.len());
            blobs.push(String::from_utf8_lossy(&bytes[start..stop]).into_owned());
            pos = stop + 1;
        }
        Ok(blobs)
    }
    /// A new commit on `parent` (or a root commit) with files written and paths removed, built in
    /// a private index so the user's staging area is never touched.
    pub async fn commit(
        &self,
        parent: Option<&str>,
        writes: &[(String, String)],
        removes: &[String],
        message: &str,
    ) -> Result<String> {
        let env = [("GIT_INDEX_FILE", self.index.as_path())];
        match parent {
            Some(parent) => self.run(&["read-tree", parent], None, &env).await?,
            None => self.run(&["read-tree", "--empty"], None, &env).await?,
        };
        let mut info = String::new();
        for (path, content) in writes {
            safe_path(path)?;
            let sha = self
                .run(
                    &["hash-object", "-w", "--stdin"],
                    Some(content.as_bytes()),
                    &[],
                )
                .await?;
            info.push_str(&format!("100644 {}\t{path}\n", sha.trim()));
        }
        for path in removes {
            safe_path(path)?;
            info.push_str(&format!("0 {}\t{path}\n", "0".repeat(40)));
        }
        if !info.is_empty() {
            self.run(
                &["update-index", "--index-info"],
                Some(info.as_bytes()),
                &env,
            )
            .await?;
        }
        let tree = self.run(&["write-tree"], None, &env).await?;
        let mut args = vec!["commit-tree", tree.trim(), "-m", message];
        if let Some(parent) = parent {
            args.extend(["-p", parent]);
        }
        Ok(self.run(&args, None, &[]).await?.trim().to_owned())
    }
    /// Compare-and-swap push: succeeds only if the remote branch is still at `expected` (or still
    /// absent when None). Hooks are skipped because this branch is not code.
    pub async fn push(
        &self,
        remote: &str,
        sha: &str,
        expected: Option<&str>,
        force: bool,
    ) -> Result<Push> {
        valid_remote(remote)?;
        let lease = format!("--force-with-lease={REMOTE_REF}:{}", expected.unwrap_or(""));
        let refspec = format!("{}{sha}:{REMOTE_REF}", if force { "+" } else { "" });
        match self
            .run(
                &[
                    "push",
                    "--porcelain",
                    "--no-verify",
                    "--quiet",
                    &lease,
                    remote,
                    &refspec,
                ],
                None,
                &[],
            )
            .await
        {
            Ok(_) => {
                self.run(&["update-ref", LOCAL_REF, sha], None, &[]).await?;
                Ok(Push::Pushed)
            }
            Err(AppError::Validation(message))
                if ["stale info", "rejected", "fetch first", "non-fast-forward"]
                    .iter()
                    .any(|m| message.contains(m)) =>
            {
                Ok(Push::Stale)
            }
            Err(error) => Err(error),
        }
    }
    pub async fn commit_count(&self, commit: &str) -> Result<u64> {
        Ok(self
            .run(&["rev-list", "--count", commit], None, &[])
            .await?
            .trim()
            .parse()
            .unwrap_or(0))
    }
    pub async fn delete_remote_branch(&self, remote: &str, expected: &str) -> Result<()> {
        valid_remote(remote)?;
        let lease = format!("--force-with-lease={REMOTE_REF}:{expected}");
        self.run(
            &[
                "push",
                "--no-verify",
                "--quiet",
                &lease,
                remote,
                &format!(":{REMOTE_REF}"),
            ],
            None,
            &[],
        )
        .await
        .map(|_| ())
    }
}

/// Remote names come from `git remote`; refuse anything that could be read as an option or URL.
fn valid_remote(remote: &str) -> Result<()> {
    if remote.is_empty()
        || remote.starts_with('-')
        || remote.len() > 100
        || !remote
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-/".contains(c))
    {
        return Err(AppError::Validation(
            "Choose one of the repository's remotes.".into(),
        ));
    }
    Ok(())
}
fn safe_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.starts_with('/')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || path.chars().any(|c| c.is_control())
    {
        return Err(AppError::Validation("Invalid sync path.".into()));
    }
    Ok(())
}
/// Strip credentials from any URL that git echoes back in an error.
fn redact_text(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            let trimmed = word.trim_matches(['\'', '"']);
            if trimmed.contains("://") {
                word.replace(trimmed, &redact(trimmed))
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_never_leave_a_url() {
        assert_eq!(
            redact("https://bob:s3cret@github.com/a/b.git"),
            "https://github.com/a/b.git"
        );
        assert_eq!(redact("git@github.com:a/b.git"), "git@github.com:a/b.git");
        assert!(
            !redact_text("fatal: unable to access 'https://x:tok@h.com/r/': 403").contains("tok")
        );
        assert!(valid_remote("--upload-pack=x").is_err());
        assert!(valid_remote("origin").is_ok());
        assert!(safe_path("events/../x").is_err());
    }
}
