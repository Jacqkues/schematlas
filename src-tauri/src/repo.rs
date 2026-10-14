//! The `.schematlas/` folder inside a repository: path safety, listing, compare-and-swap writes and
//! read-only code snippets. Nothing here runs git or follows a symlink out of the repository.
use crate::{
    diagram::{self, CodeRef, Diagram},
    error::{AppError, Result},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

pub const ATLAS_DIR: &str = ".schematlas";
const MAX_DOC: usize = 1024 * 1024;
const MAX_CODE_FILE: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Diagram,
    Doc,
}
impl Kind {
    fn folder(self) -> &'static str {
        match self {
            Kind::Diagram => "diagrams",
            Kind::Doc => "docs",
        }
    }
    fn extension(self) -> &'static str {
        match self {
            Kind::Diagram => "json",
            Kind::Doc => "md",
        }
    }
    pub fn relative(self, slug: &str) -> String {
        format!("{ATLAS_DIR}/{}/{slug}.{}", self.folder(), self.extension())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub slug: String,
    pub path: String,
    pub title: String,
    pub hash: String,
    /// Diagram kind, e.g. `architecture`; None for docs.
    pub kind: Option<String>,
    pub nodes: usize,
    pub edges: usize,
    /// Files that fail validation are listed with the reason instead of disappearing.
    pub error: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub root: Option<String>,
    pub is_git: bool,
    pub diagrams: Vec<Entry>,
    pub docs: Vec<Entry>,
}
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
    /// `.schematlas/<folder>`, refusing symlinks so a repository cannot redirect writes elsewhere.
    fn folder(&self, kind: Kind, create: bool) -> Result<Option<PathBuf>> {
        let mut path = self.root.clone();
        for part in [ATLAS_DIR, kind.folder()] {
            path.push(part);
            match fs::symlink_metadata(&path) {
                Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                    return Err(AppError::Validation(format!(
                        "{} must be a plain folder inside the repository.",
                        path.strip_prefix(&self.root).unwrap_or(&path).display()
                    )));
                }
                Ok(_) => {}
                Err(e) if e.kind() == ErrorKind::NotFound && create => fs::create_dir(&path)?,
                Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(Some(path))
    }
    fn file(&self, kind: Kind, slug: &str, create: bool) -> Result<Option<PathBuf>> {
        diagram::valid_slug(slug)?;
        Ok(self
            .folder(kind, create)?
            .map(|dir| dir.join(format!("{slug}.{}", kind.extension()))))
    }
    /// The file's text and hash, or None when it does not exist.
    pub fn read(&self, kind: Kind, slug: &str) -> Result<Option<(String, String)>> {
        let Some(path) = self.file(kind, slug, false)? else {
            return Ok(None);
        };
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_file() => {
                return Err(AppError::Validation(format!(
                    "{} is not a plain file.",
                    kind.relative(slug)
                )))
            }
            Ok(meta) if meta.len() as usize > diagram::MAX_FILE.max(MAX_DOC) => {
                return Err(AppError::Validation(format!(
                    "{} is too large.",
                    kind.relative(slug)
                )))
            }
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        }
        let bytes = fs::read(&path)?;
        let digest = hash(&bytes);
        let text = String::from_utf8(bytes).map_err(|_| {
            AppError::Validation(format!("{} is not UTF-8 text.", kind.relative(slug)))
        })?;
        Ok(Some((text, digest)))
    }
    /// Compare-and-swap: `expected` None means the file must not exist yet; Some(hash) means it
    /// must still have that content. Callers serialize writes, so the check cannot race us.
    pub fn write(
        &self,
        kind: Kind,
        slug: &str,
        text: &str,
        expected: Option<&str>,
    ) -> Result<String> {
        if kind == Kind::Doc && text.len() > MAX_DOC {
            return Err(AppError::Validation("Docs are limited to 1 MB.".into()));
        }
        let current = self.read(kind, slug)?.map(|(_, h)| h);
        if current.as_deref() != expected {
            return Err(conflict(kind, slug));
        }
        let path = self.file(kind, slug, true)?.ok_or(AppError::NotFound)?;
        let dir = path.parent().ok_or(AppError::NotFound)?;
        let temp = dir.join(format!(".{slug}.{}.tmp", uuid::Uuid::new_v4().simple()));
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
    pub fn delete(&self, kind: Kind, slug: &str, expected: &str) -> Result<()> {
        let current = self.read(kind, slug)?.map(|(_, h)| h);
        if current.as_deref() != Some(expected) {
            return Err(conflict(kind, slug));
        }
        if let Some(path) = self.file(kind, slug, false)? {
            fs::remove_file(path)?;
        }
        Ok(())
    }
    pub fn list(&self) -> Result<Listing> {
        let mut listing = Listing {
            root: Some(self.root.to_string_lossy().into_owned()),
            is_git: self.is_git(),
            ..Listing::default()
        };
        for kind in [Kind::Diagram, Kind::Doc] {
            let Some(dir) = self.folder(kind, false)? else {
                continue;
            };
            let mut entries = vec![];
            for item in fs::read_dir(dir)? {
                let item = item?;
                let name = item.file_name().to_string_lossy().into_owned();
                let Some(slug) = name.strip_suffix(&format!(".{}", kind.extension())) else {
                    continue;
                };
                if diagram::valid_slug(slug).is_err() {
                    continue;
                }
                entries.push(match self.read(kind, slug) {
                    Ok(Some((text, digest))) => describe(kind, slug, &text, digest),
                    Ok(None) => continue,
                    Err(e) => Entry {
                        slug: slug.into(),
                        path: kind.relative(slug),
                        title: slug.into(),
                        hash: String::new(),
                        kind: None,
                        nodes: 0,
                        edges: 0,
                        error: Some(e.to_string()),
                    },
                });
            }
            entries.sort_by_key(|e| e.title.to_lowercase());
            match kind {
                Kind::Diagram => listing.diagrams = entries,
                Kind::Doc => listing.docs = entries,
            }
        }
        Ok(listing)
    }
    /// The absolute path of a code reference, which must be an existing file inside the repo.
    pub fn code_path(&self, code_ref: &CodeRef) -> Result<PathBuf> {
        code_ref.validate()?;
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

fn conflict(kind: Kind, slug: &str) -> AppError {
    AppError::Validation(format!(
        "{} changed since it was read. Reload it and try again.",
        kind.relative(slug)
    ))
}

fn describe(kind: Kind, slug: &str, text: &str, digest: String) -> Entry {
    let mut entry = Entry {
        slug: slug.into(),
        path: kind.relative(slug),
        title: slug.into(),
        hash: digest,
        kind: None,
        nodes: 0,
        edges: 0,
        error: None,
    };
    match kind {
        Kind::Diagram => match Diagram::parse(text) {
            Ok(d) => {
                entry.title = d.title;
                entry.kind = Some(d.kind);
                entry.nodes = d.nodes.len();
                entry.edges = d.edges.len();
            }
            Err(e) => entry.error = Some(e.to_string()),
        },
        Kind::Doc => entry.title = doc_title(text).unwrap_or_else(|| slug.into()),
    }
    entry
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
    const DIAGRAM: &str = r#"{"title":"Map","nodes":[{"id":"a","label":"A"}]}"#;

    #[test]
    fn writes_are_compare_and_swap() {
        let (_dir, repo) = repo();
        assert!(repo.list().unwrap().diagrams.is_empty());
        let first = repo.write(Kind::Diagram, "map", DIAGRAM, None).unwrap();
        // Creating again, or saving against a stale hash, is a conflict.
        assert!(repo.write(Kind::Diagram, "map", DIAGRAM, None).is_err());
        assert!(repo
            .write(Kind::Diagram, "map", DIAGRAM, Some("stale"))
            .is_err());
        let second = repo
            .write(Kind::Diagram, "map", "{}", Some(&first))
            .unwrap();
        let listing = repo.list().unwrap();
        assert_eq!(listing.diagrams.len(), 1);
        assert!(
            listing.diagrams[0].error.is_some(),
            "invalid files stay listed"
        );
        assert!(repo.delete(Kind::Diagram, "map", &first).is_err());
        repo.delete(Kind::Diagram, "map", &second).unwrap();
        assert!(repo.read(Kind::Diagram, "map").unwrap().is_none());
        // No temporary files are left behind.
        let leftovers = fs::read_dir(repo.root().join(".schematlas/diagrams"))
            .unwrap()
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn slugs_and_refs_cannot_escape() {
        let (dir, repo) = repo();
        assert!(repo.write(Kind::Doc, "../x", "", None).is_err());
        fs::write(dir.path().join("main.rs"), "fn main() {}\n// two\n").unwrap();
        let snippet = repo.snippet(&CodeRef::parse("main.rs:2"), 5).unwrap();
        assert_eq!(snippet.lines, ["fn main() {}", "// two"]);
        assert!(repo.snippet(&CodeRef::parse("main.rs:9"), 0).is_err());
        assert!(repo.code_path(&CodeRef::parse("../outside")).is_err());
        assert!(repo.code_path(&CodeRef::parse("missing.rs")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_atlas_folders_are_refused() {
        let (dir, repo) = repo();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(ATLAS_DIR)).unwrap();
        assert!(repo.write(Kind::Diagram, "map", DIAGRAM, None).is_err());
        assert!(repo.list().is_err());
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
        let link = dir.path().join("link.rs");
        std::os::unix::fs::symlink("/etc/hosts", &link).unwrap();
        assert!(repo.code_path(&CodeRef::parse("link.rs")).is_err());
    }

    #[test]
    fn doc_titles() {
        assert_eq!(
            doc_title("---\ntitle: \"Intro\"\n---\n# Other").as_deref(),
            Some("Intro")
        );
        assert_eq!(doc_title("text\n# Heading\n").as_deref(), Some("Heading"));
        assert_eq!(doc_title("nothing"), None);
    }
}
