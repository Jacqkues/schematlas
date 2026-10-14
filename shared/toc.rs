//! Portable documentation navigation. Paths are relative to `.schematlas/`.
//! IDs used by the UI are derived from file references or folder ancestry, never persisted.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub const VERSION: u32 = 1;
pub const MAX_ITEMS: usize = 2000;
pub const MAX_DEPTH: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toc {
    pub version: u32,
    pub items: Vec<Entry>,
}
impl Default for Toc {
    fn default() -> Self {
        Self {
            version: VERSION,
            items: vec![],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// A local source name, for legacy connected sources. Credentials are never part of the TOC.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<Entry>>,
}
impl Entry {
    pub fn folder(title: String) -> Self {
        Self {
            title,
            file: None,
            source: None,
            kind: None,
            block: None,
            page: None,
            children: Some(vec![]),
        }
    }
    pub fn file(title: String, file: String) -> Self {
        Self {
            title,
            file: Some(file),
            source: None,
            kind: None,
            block: None,
            page: None,
            children: None,
        }
    }
    pub fn source(title: String, source: String, kind: String) -> Self {
        Self {
            title,
            file: None,
            source: Some(source),
            kind: Some(kind),
            block: None,
            page: None,
            children: None,
        }
    }
    pub fn is_folder(&self) -> bool {
        self.children.is_some()
    }
    pub fn is_markdown(&self) -> bool {
        self.block.is_none()
            && self
                .file
                .as_ref()
                .is_some_and(|p| p.ends_with(".md") || p.ends_with(".markdown"))
    }
    /// Adapter for the standard-file parser's repository-relative `path#part` identifiers.
    pub fn document_id(&self) -> Option<String> {
        let path = format!(".schematlas/{}", self.file.as_ref()?);
        Some(match self.block.or(self.page) {
            Some(n) => format!("{path}#{n}"),
            None => path,
        })
    }
    fn id(&self, parents: &[String]) -> String {
        if self.is_folder() {
            let mut path = parents.to_vec();
            path.push(self.title.clone());
            format!(
                "folder:{}",
                serde_json::to_string(&path).unwrap_or_default()
            )
        } else if let Some(file) = &self.file {
            format!(
                "file:{}",
                serde_json::to_string(&(file, self.block, self.page)).unwrap_or_default()
            )
        } else {
            format!(
                "source:{}",
                serde_json::to_string(&(&self.source, &self.kind)).unwrap_or_default()
            )
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub parent: Option<String>,
    pub path: String,
    pub entry: Entry,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub root: Option<String>,
    pub toc: Toc,
    pub hash: Option<String>,
    #[serde(default)]
    pub issues: BTreeMap<String, String>,
    #[serde(default)]
    pub hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum Edit {
    Create {
        title: String,
        parent: Option<String>,
    },
    Update {
        id: String,
        title: String,
        parent: Option<String>,
    },
    Move {
        id: String,
        parent: Option<String>,
        #[serde(default)]
        before: Option<String>,
    },
    Remove {
        id: String,
    },
    Add {
        entry: Entry,
        parent: Option<String>,
    },
}

pub fn valid_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 512
        || path.contains(['\\', ':', '#'])
        || path.chars().any(char::is_control)
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || path.starts_with(".schematlas/")
        || path == "toc.yaml"
        || path.starts_with("layout/")
    {
        return Err("File paths must stay inside .schematlas/ and must not point to toc.yaml or layout files.".into());
    }
    Ok(())
}
impl Toc {
    pub fn rows(&self) -> Vec<Row> {
        fn walk(
            items: &[Entry],
            parents: &[String],
            parent: Option<String>,
            result: &mut Vec<Row>,
            depth: usize,
        ) {
            if depth > MAX_DEPTH {
                return;
            }
            for item in items {
                let id = item.id(parents);
                let mut path = parents.to_vec();
                path.push(item.title.clone());
                result.push(Row {
                    id: id.clone(),
                    parent: parent.clone(),
                    path: path.join(" / "),
                    entry: item.clone(),
                });
                if let Some(children) = &item.children {
                    walk(children, &path, Some(id), result, depth + 1);
                }
            }
        }
        let mut result = vec![];
        walk(&self.items, &[], None, &mut result, 0);
        result
    }
    pub fn within(&self, id: &str, ancestor: &str) -> bool {
        let rows = self.rows();
        let mut current = Some(id);
        for _ in 0..=MAX_DEPTH {
            let Some(id) = current else { return false };
            if id == ancestor {
                return true;
            }
            current = rows
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.parent.as_deref());
        }
        false
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != VERSION {
            return Err(format!(
                "Unsupported toc.yaml version {}. Expected 1.",
                self.version
            ));
        }
        fn walk(
            items: &[Entry],
            parents: &[String],
            ids: &mut HashSet<String>,
            count: &mut usize,
        ) -> Result<(), String> {
            if parents.len() > MAX_DEPTH {
                return Err("Documentation folders can be nested up to eight levels deep.".into());
            }
            let mut titles = HashSet::new();
            for item in items {
                *count += 1;
                if *count > MAX_ITEMS {
                    return Err("toc.yaml can contain up to 2,000 entries.".into());
                }
                if item.title.trim().is_empty()
                    || item.title.chars().count() > 120
                    || item.title.chars().any(char::is_control)
                {
                    return Err("Each TOC entry needs a title of 1–120 characters.".into());
                }
                if !ids.insert(item.id(parents)) {
                    return Err(format!("Duplicate TOC entry: {}.", item.title));
                }
                match (&item.file, &item.source, &item.children) {
                    (None, None, Some(children)) => {
                        if item.kind.is_some() || item.block.is_some() || item.page.is_some() {
                            return Err("Folders only have a title and children.".into());
                        }
                        if !titles.insert(item.title.to_lowercase()) {
                            return Err("A folder with this name already exists here.".into());
                        }
                        let mut path = parents.to_vec();
                        path.push(item.title.clone());
                        walk(children, &path, ids, count)?;
                    }
                    (Some(file), None, None) => {
                        valid_path(file)?;
                        if item.block.is_some()
                            && !(file.ends_with(".md") || file.ends_with(".markdown"))
                        {
                            return Err("A block selector requires a Markdown file.".into());
                        }
                        if item.page.is_some()
                            && !(file.ends_with(".drawio") || file.ends_with(".dio"))
                        {
                            return Err("A page selector requires a draw.io file.".into());
                        }
                    }
                    (None, Some(source), None) => {
                        if source.trim().is_empty()
                            || source.len() > 240
                            || !matches!(item.kind.as_deref(), Some("database" | "openapi"))
                            || item.block.is_some()
                            || item.page.is_some()
                        {
                            return Err("A local source reference needs a name and type: database or openapi.".into());
                        }
                    }
                    _ => {
                        return Err(
                            "Each TOC entry must contain exactly one of file, children, or source."
                                .into(),
                        )
                    }
                }
                if item.block.is_some() && item.page.is_some() {
                    return Err("Use either block or page, not both.".into());
                }
                if item
                    .kind
                    .as_deref()
                    .is_some_and(|k| !matches!(k, "doc" | "diagram" | "database" | "openapi"))
                {
                    return Err("Unknown TOC entry type.".into());
                }
            }
            Ok(())
        }
        walk(&self.items, &[], &mut HashSet::new(), &mut 0)
    }
    pub fn apply(&mut self, edit: Edit) -> Result<(), String> {
        self.validate()?;
        let mut next = self.clone();
        match edit {
            Edit::Create { title, parent } => {
                next.insert(Entry::folder(title.trim().into()), parent.as_deref(), None)?
            }
            Edit::Add { entry, parent } => next.insert(entry, parent.as_deref(), None)?,
            Edit::Remove { id } => {
                let removed = next.take(&id)?;
                if removed.children.as_ref().is_some_and(|c| !c.is_empty()) {
                    return Err("Move this folder’s contents elsewhere before deleting it.".into());
                }
            }
            Edit::Move { id, parent, before } => {
                if parent.as_deref().is_some_and(|p| self.within(p, &id)) {
                    return Err("A folder cannot be placed inside itself.".into());
                }
                if before.as_deref() == Some(&id) {
                    return Ok(());
                }
                let item = next.take(&id)?;
                next.insert(item, parent.as_deref(), before.as_deref())?;
            }
            Edit::Update { id, title, parent } => {
                if parent.as_deref().is_some_and(|p| self.within(p, &id)) {
                    return Err("A folder cannot be placed inside itself.".into());
                }
                let row = self
                    .rows()
                    .into_iter()
                    .find(|r| r.id == id)
                    .ok_or("Entry no longer exists.")?;
                let before = self
                    .rows()
                    .into_iter()
                    .filter(|r| r.parent == parent)
                    .skip_while(|r| r.id != id)
                    .nth(1)
                    .map(|r| r.id);
                let mut item = next.take(&id)?;
                item.title = title.trim().into();
                next.insert(
                    item,
                    parent.as_deref(),
                    if row.parent == parent {
                        before.as_deref()
                    } else {
                        None
                    },
                )?;
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    fn take(&mut self, id: &str) -> Result<Entry, String> {
        fn walk(items: &mut Vec<Entry>, parents: &[String], id: &str) -> Option<Entry> {
            if let Some(i) = items.iter().position(|e| e.id(parents) == id) {
                return Some(items.remove(i));
            }
            for item in items {
                let mut path = parents.to_vec();
                path.push(item.title.clone());
                if let Some(children) = &mut item.children {
                    if let Some(found) = walk(children, &path, id) {
                        return Some(found);
                    }
                }
            }
            None
        }
        walk(&mut self.items, &[], id).ok_or_else(|| "Entry no longer exists.".into())
    }
    fn insert(
        &mut self,
        entry: Entry,
        parent: Option<&str>,
        before: Option<&str>,
    ) -> Result<(), String> {
        fn folder<'a>(
            items: &'a mut Vec<Entry>,
            parents: &[String],
            id: Option<&str>,
        ) -> Option<(&'a mut Vec<Entry>, Vec<String>)> {
            let Some(id) = id else {
                return Some((items, parents.to_vec()));
            };
            for item in items {
                let matches = item.id(parents) == id;
                let mut path = parents.to_vec();
                path.push(item.title.clone());
                if let Some(children) = &mut item.children {
                    if matches {
                        return Some((children, path));
                    }
                    if let Some(found) = folder(children, &path, Some(id)) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let (items, parents) =
            folder(&mut self.items, &[], parent).ok_or("Destination folder no longer exists.")?;
        let index = match before {
            Some(id) => items
                .iter()
                .position(|e| e.id(&parents) == id)
                .ok_or("The entry to insert before is not in this folder.")?,
            None => items.len(),
        };
        items.insert(index, entry);
        Ok(())
    }
}

/// Deterministic YAML for this small schema. JSON string escaping is valid YAML escaping.
/// Keeping this writer shared gives the browser preview the same file hashes and format.
pub fn to_yaml(toc: &Toc) -> String {
    fn quoted(s: &str) -> String {
        serde_json::to_string(s).unwrap_or_default()
    }
    fn entries(items: &[Entry], indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        for e in items {
            out.push_str(&format!("{pad}- title: {}\n", quoted(&e.title)));
            for (key, value) in [("file", &e.file), ("source", &e.source), ("type", &e.kind)] {
                if let Some(value) = value {
                    out.push_str(&format!("{pad}  {key}: {}\n", quoted(value)));
                }
            }
            for (key, value) in [("block", e.block), ("page", e.page)] {
                if let Some(value) = value {
                    out.push_str(&format!("{pad}  {key}: {value}\n"));
                }
            }
            if let Some(children) = &e.children {
                if children.is_empty() {
                    out.push_str(&format!("{pad}  children: []\n"));
                } else {
                    out.push_str(&format!("{pad}  children:\n"));
                    entries(children, indent + 4, out);
                }
            }
        }
    }
    let mut out = format!("version: {}\n", toc.version);
    if toc.items.is_empty() {
        out.push_str("items: []\n");
    } else {
        out.push_str("items:\n");
        entries(&toc.items, 2, &mut out);
    }
    out
}

/// Convert the old local explorer metadata once. New TOCs have no predefined sections.
pub fn migrate(explorer: &crate::explorer::Explorer, items: &[(String, String, Entry)]) -> Toc {
    fn children(
        parent: Option<&str>,
        explorer: &crate::explorer::Explorer,
        items: &[(String, String, Entry)],
        depth: usize,
    ) -> Vec<Entry> {
        if depth > MAX_DEPTH {
            return vec![];
        }
        let mut result = vec![];
        let mut folders = explorer
            .folders
            .iter()
            .filter(|f| f.parent.as_deref() == parent)
            .collect::<Vec<_>>();
        folders.sort_by_key(|f| f.name.to_lowercase());
        for folder in folders {
            let nested = children(Some(&folder.id), explorer, items, depth + 1);
            let builtin = ["diagrams", "docs", "databases", "apis"].contains(&folder.id.as_str());
            if !nested.is_empty() || !builtin || *explorer != crate::explorer::Explorer::default() {
                let mut entry = Entry::folder(folder.name.clone());
                entry.children = Some(nested);
                result.push(entry);
            }
        }
        let mut files = items
            .iter()
            .filter(|(id, default, _)| explorer.location(id, default).as_deref() == parent)
            .map(|(_, _, e)| e.clone())
            .collect::<Vec<_>>();
        files.sort_by_key(|e| e.title.to_lowercase());
        result.extend(files);
        result
    }
    Toc {
        version: VERSION,
        items: children(None, explorer, items, 0),
    }
}

pub fn document_path(toc: &Toc, parent: Option<&str>, title: &str) -> Result<String, String> {
    fn slug(text: &str) -> String {
        let s = text
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c.to_lowercase().collect::<String>()
                } else {
                    "-".into()
                }
            })
            .collect::<String>();
        let value = s.trim_matches('-').chars().take(80).collect::<String>();
        if value.is_empty() {
            "untitled".into()
        } else {
            value
        }
    }
    let rows = toc.rows();
    let mut segments = vec![format!("{}.md", slug(title))];
    let mut current = parent;
    for _ in 0..=MAX_DEPTH {
        let Some(id) = current else {
            segments.reverse();
            return Ok(segments.join("/"));
        };
        let row = rows
            .iter()
            .find(|r| r.id == id && r.entry.is_folder())
            .ok_or("Destination folder no longer exists.")?;
        segments.push(slug(&row.entry.title));
        current = row.parent.as_deref();
    }
    Err("Folder nesting is too deep.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordering_and_folders_survive_edits_without_persisted_ids() {
        let mut toc = Toc {
            version: 1,
            items: vec![
                Entry::file("Intro".into(), "intro.md".into()),
                Entry::folder("Architecture".into()),
                Entry::file("Map".into(), "system.mmd".into()),
            ],
        };
        let rows = toc.rows();
        toc.apply(Edit::Move {
            id: rows[2].id.clone(),
            parent: Some(rows[1].id.clone()),
            before: None,
        })
        .unwrap();
        assert_eq!(toc.rows()[2].parent, Some(rows[1].id.clone()));
        let before = toc.clone();
        assert!(toc
            .apply(Edit::Move {
                id: rows[1].id.clone(),
                parent: Some(rows[1].id.clone()),
                before: None
            })
            .is_err());
        assert_eq!(toc, before);
        toc.apply(Edit::Update {
            id: rows[1].id.clone(),
            title: "System".into(),
            parent: None,
        })
        .unwrap();
        assert_eq!(toc.rows()[2].path, "System / Map");
        assert_eq!(toc.items[0].title, "Intro");
        assert!(!serde_json::to_string(&toc).unwrap().contains("folder:"));
        let rows = toc.rows();
        toc.apply(Edit::Move {
            id: rows[1].id.clone(),
            parent: None,
            before: Some(rows[0].id.clone()),
        })
        .unwrap();
        assert_eq!(toc.items[0].title, "System");
    }
    #[test]
    fn validates_paths_selectors_and_duplicate_references() {
        for path in [
            "../outside.md",
            "/tmp/x.md",
            "a/../x.md",
            "a\\x.md",
            "https://x.md",
            "toc.yaml",
            "layout/x.json",
        ] {
            assert!(valid_path(path).is_err());
        }
        let mut entry = Entry::file("Flow".into(), "guide.md".into());
        entry.block = Some(2);
        let mut toc = Toc {
            version: 1,
            items: vec![entry.clone()],
        };
        toc.validate().unwrap();
        assert_eq!(
            entry.document_id().as_deref(),
            Some(".schematlas/guide.md#2")
        );
        toc.items.push(entry);
        assert!(toc.validate().is_err());
        toc.items.pop();
        toc.items[0].page = Some(1);
        assert!(toc.validate().is_err());
    }
}
