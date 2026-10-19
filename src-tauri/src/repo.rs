//! Files inside a repository: path safety, discovery of diagram files, compare-and-swap writes
//! and read-only code snippets. Nothing here runs git or follows a symlink out of the repository.
use crate::error::{AppError, Result};
use schematlas_parser::model::CodeRef;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Component, Path, PathBuf},
};

/// Schematlas's own folder: docs, Mermaid layouts. Diagrams live wherever the repository keeps them.
pub const ATLAS_DIR: &str = ".schematlas";
const MAX_FILE: u64 = 2 * 1024 * 1024;
const MAX_CODE_FILE: u64 = 4 * 1024 * 1024;
/// Discovery stops here so a huge monorepo cannot stall the window.
const MAX_FILES: usize = 4000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub path: String,
    pub start: u32,
    pub focus: Option<(u32, u32)>,
    pub lines: Vec<String>,
    pub total: u32,
}

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// What a file may hold, from its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Mermaid,
    Markdown,
    Drawio,
    Dbml,
    Excalidraw,
}
impl Format {
    /// Whether the file itself holds where things are drawn (else `.schematlas/layout/` does).
    pub fn keeps_positions(self) -> bool {
        matches!(self, Format::Drawio | Format::Excalidraw)
    }
    pub fn of(path: &str) -> Option<Format> {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".mmd") || lower.ends_with(".mermaid") {
            Some(Format::Mermaid)
        } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
            Some(Format::Markdown)
        } else if lower.ends_with(".drawio")
            || lower.ends_with(".drawio.svg")
            || lower.ends_with(".dio")
        {
            Some(Format::Drawio)
        } else if lower.ends_with(".dbml") {
            Some(Format::Dbml)
        } else if lower.ends_with(".excalidraw") {
            Some(Format::Excalidraw)
        } else {
            None
        }
    }
}

/// A repository-relative path: forward slashes, no `..`, not absolute.
pub fn valid_relative(path: &str) -> Result<()> {
    let bad = path.is_empty()
        || path.len() > 512
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.chars().nth(1) == Some(':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        return Err(AppError::Validation(format!(
            "`{path}` is not a path inside the repository."
        )));
    }
    Ok(())
}

/// A canonical repository root. Every path handed out is checked to stay beneath it.
#[derive(Clone, Debug)]
pub struct Repo {
    root: PathBuf,
}
impl Repo {
    pub fn open(directory: &str) -> Result<Self> {
        let root = fs::canonicalize(directory)
            .map_err(|_| AppError::Validation("The repository folder no longer exists.".into()))?;
        if !root.is_dir() {
            return Err(AppError::Validation(
                "The repository path is not a folder.".into(),
            ));
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn is_git(&self) -> bool {
        self.root.join(".git").exists()
    }
    /// The absolute path for a relative one, refusing symlinks anywhere along the way so a
    /// repository cannot redirect reads or writes outside itself.
    fn resolve(&self, relative: &str) -> Result<PathBuf> {
        valid_relative(relative)?;
        let mut path = self.root.clone();
        for component in Path::new(relative).components() {
            let Component::Normal(part) = component else {
                return Err(AppError::Validation(format!(
                    "`{relative}` is not a path inside the repository."
                )));
            };
            path.push(part);
            if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(AppError::Validation(format!(
                    "`{relative}` goes through a symbolic link."
                )));
            }
        }
        Ok(path)
    }
    /// The file's text and hash, or None when it does not exist.
    pub fn read(&self, relative: &str) -> Result<Option<(String, String)>> {
        let path = self.resolve(relative)?;
        match fs::metadata(&path) {
            Ok(meta) if !meta.is_file() => {
                return Err(AppError::Validation(format!("`{relative}` is not a file.")))
            }
            Ok(meta) if meta.len() > MAX_FILE => {
                return Err(AppError::Validation(format!(
                    "`{relative}` is larger than 2 MB."
                )))
            }
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        let bytes = fs::read(&path)?;
        let digest = hash(&bytes);
        let text = String::from_utf8(bytes)
            .map_err(|_| AppError::Validation(format!("`{relative}` is not UTF-8 text.")))?;
        Ok(Some((text, digest)))
    }
    /// Compare-and-swap: `expected` None means the file must not exist yet; Some(hash) means it
    /// must still have that content. Callers serialize writes, so the check cannot race us.
    pub fn write(&self, relative: &str, text: &str, expected: Option<&str>) -> Result<String> {
        if text.len() as u64 > MAX_FILE {
            return Err(AppError::Validation("Files are limited to 2 MB.".into()));
        }
        let current = self.read(relative)?.map(|(_, h)| h);
        if current.as_deref() != expected {
            return Err(conflict(relative));
        }
        let path = self.resolve(relative)?;
        let dir = path.parent().ok_or(AppError::NotFound)?;
        fs::create_dir_all(dir)?;
        // Created folders are checked again: create_dir_all follows links planted meanwhile.
        if !fs::canonicalize(dir)?.starts_with(&self.root) {
            return Err(AppError::Validation(format!(
                "`{relative}` is outside the repository."
            )));
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let temp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4().simple()));
        let written = (|| {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, &path)
        })();
        if let Err(e) = written {
            let _ = fs::remove_file(&temp);
            return Err(e.into());
        }
        Ok(hash(text.as_bytes()))
    }
    pub fn delete(&self, relative: &str, expected: &str) -> Result<()> {
        let current = self.read(relative)?.map(|(_, h)| h);
        if current.as_deref() != Some(expected) {
            return Err(conflict(relative));
        }
        fs::remove_file(self.resolve(relative)?)?;
        Ok(())
    }
    /// Repository-relative paths of files that may hold diagrams, honouring `.gitignore`.
    pub fn discover(&self) -> Vec<String> {
        let mut found = vec![];
        let walker = ignore::WalkBuilder::new(&self.root)
            .hidden(false)
            .follow_links(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git" && e.file_name() != "node_modules")
            .build();
        for entry in walker.flatten() {
            if found.len() >= MAX_FILES {
                break;
            }
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Ok(relative) = entry.path().strip_prefix(&self.root) else {
                continue;
            };
            let relative = relative.to_string_lossy().replace('\\', "/");
            if relative.starts_with(&format!("{ATLAS_DIR}/layout/")) {
                continue;
            }
            if Format::of(&relative).is_some()
                && entry.metadata().is_ok_and(|m| m.len() <= MAX_FILE)
            {
                found.push(relative);
            }
        }
        found.sort();
        found
    }
    /// Every file the repository tracks or could (honouring `.gitignore`), with its size, up to
    /// `limit` files: what an agent sees before reading anything.
    pub fn files(&self, limit: usize) -> Vec<(String, u64)> {
        let mut found = vec![];
        let walker = ignore::WalkBuilder::new(&self.root)
            .hidden(false)
            .follow_links(false)
            .require_git(false)
            .filter_entry(|e| e.file_name() != ".git" && e.file_name() != "node_modules")
            .build();
        for entry in walker.flatten() {
            if found.len() >= limit {
                break;
            }
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let Ok(relative) = entry.path().strip_prefix(&self.root) else {
                continue;
            };
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            found.push((relative.to_string_lossy().replace('\\', "/"), size));
        }
        found.sort();
        found
    }
    /// The commit `HEAD` points at, read from `.git` without running git.
    pub fn head_commit(&self) -> Option<String> {
        let git = self.root.join(".git");
        let head = fs::read_to_string(git.join("HEAD")).ok()?;
        let head = head.trim();
        let Some(reference) = head.strip_prefix("ref: ") else {
            return (head.len() == 40).then(|| head.to_owned());
        };
        if !reference.starts_with("refs/") || reference.contains("..") {
            return None;
        }
        if let Ok(hash) = fs::read_to_string(git.join(reference)) {
            return Some(hash.trim().to_owned());
        }
        fs::read_to_string(git.join("packed-refs"))
            .ok()?
            .lines()
            .find_map(|line| {
                let (hash, name) = line.split_once(' ')?;
                (name == reference).then(|| hash.to_owned())
            })
    }
    /// The absolute path of a code reference, which must be an existing file inside the repo.
    pub fn code_path(&self, code_ref: &CodeRef) -> Result<PathBuf> {
        code_ref
            .validate()
            .map_err(|e| AppError::Validation(e.to_string()))?;
        let missing = || {
            AppError::Validation(format!(
                "`{}` does not exist in the repository.",
                code_ref.path
            ))
        };
        let path = fs::canonicalize(self.root.join(&code_ref.path)).map_err(|_| missing())?;
        if !path.starts_with(&self.root) || !path.is_file() {
            return Err(missing());
        }
        Ok(path)
    }
    /// Lines around a reference, at most 200, for the inspector and broken-ref checks.
    pub fn snippet(&self, code_ref: &CodeRef, context: u32) -> Result<Snippet> {
        let path = self.code_path(code_ref)?;
        if fs::metadata(&path)?.len() > MAX_CODE_FILE {
            return Err(AppError::Validation(format!(
                "`{}` is too large to preview.",
                code_ref.path
            )));
        }
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();
        let total = lines.len() as u32;
        let focus = code_ref
            .line
            .map(|line| (line, code_ref.end_line.unwrap_or(line)));
        if let Some((line, _)) = focus {
            if line > total.max(1) {
                return Err(AppError::Validation(format!(
                    "`{}` has {total} lines; line {line} is past the end.",
                    code_ref.path
                )));
            }
        }
        let (first, last) = match focus {
            Some((a, b)) => (a.saturating_sub(context).max(1), b.saturating_add(context)),
            None => (1, 40),
        };
        let last = last.min(total).min(first + 199);
        Ok(Snippet {
            path: code_ref.path.clone(),
            start: first,
            focus,
            lines: lines
                .iter()
                .skip(first as usize - 1)
                .take((last + 1).saturating_sub(first) as usize)
                .map(|l| l.chars().take(400).collect())
                .collect(),
            total,
        })
    }
}

fn conflict(relative: &str) -> AppError {
    AppError::Validation(format!(
        "{relative} changed since it was read. Reload it and try again."
    ))
}

/// `title:` from front matter, else the first `# heading`.
pub fn doc_title(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if text.starts_with("---") {
        lines.next();
        for line in lines.by_ref() {
            if line.trim() == "---" {
                break;
            }
            if let Some(title) = line.strip_prefix("title:") {
                let title = title.trim().trim_matches(['"', '\'']).trim();
                if !title.is_empty() {
                    return Some(title.chars().take(120).collect());
                }
            }
        }
    }
    lines
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().chars().take(120).collect())
        .filter(|t: &String| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> (tempfile::TempDir, Repo) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo::open(dir.path().to_str().unwrap()).unwrap();
        (dir, repo)
    }

    #[test]
    fn writes_are_compare_and_swap_and_leave_no_temporary_files() {
        let (dir, repo) = repo();
        let first = repo
            .write("docs/map.mmd", "flowchart LR\n  a", None)
            .unwrap();
        assert!(repo.write("docs/map.mmd", "x", None).is_err());
        assert!(repo.write("docs/map.mmd", "x", Some("stale")).is_err());
        let second = repo
            .write("docs/map.mmd", "flowchart LR\n  b", Some(&first))
            .unwrap();
        assert!(repo.delete("docs/map.mmd", &first).is_err());
        repo.delete("docs/map.mmd", &second).unwrap();
        assert_eq!(fs::read_dir(dir.path().join("docs")).unwrap().count(), 0);
    }

    #[test]
    fn paths_cannot_escape_and_discovery_honours_gitignore() {
        let (dir, repo) = repo();
        for bad in [
            "../x.mmd",
            "/etc/x.mmd",
            "a//b.mmd",
            "a/./b.mmd",
            "C:/x.mmd",
        ] {
            assert!(repo.write(bad, "x", None).is_err(), "{bad}");
        }
        fs::write(dir.path().join(".gitignore"), "build/\n").unwrap();
        for file in [
            "docs/a.mmd",
            "README.md",
            "infra/net.drawio",
            "db/schema.dbml",
            "build/out.mmd",
            "src/main.rs",
            ".schematlas/layout/x.json",
        ] {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        }
        assert_eq!(
            repo.discover(),
            [
                "README.md",
                "db/schema.dbml",
                "docs/a.mmd",
                "infra/net.drawio"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused() {
        let (dir, repo) = repo();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join("docs")).unwrap();
        assert!(repo.write("docs/map.mmd", "x", None).is_err());
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
        std::os::unix::fs::symlink("/etc/hosts", dir.path().join("link.rs")).unwrap();
        assert!(repo.code_path(&CodeRef::parse("link.rs")).is_err());
    }

    #[test]
    fn snippets_and_titles() {
        let (dir, repo) = repo();
        fs::write(dir.path().join("main.rs"), "fn main() {}\n// two\n").unwrap();
        assert_eq!(
            repo.snippet(&CodeRef::parse("main.rs:2"), 5).unwrap().lines,
            ["fn main() {}", "// two"]
        );
        assert!(repo.snippet(&CodeRef::parse("main.rs:9"), 0).is_err());
        assert_eq!(
            doc_title("---\ntitle: \"Intro\"\n---\n# Other").as_deref(),
            Some("Intro")
        );
        assert_eq!(doc_title("text\n# Heading\n").as_deref(), Some("Heading"));
    }
}
