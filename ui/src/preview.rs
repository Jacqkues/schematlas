//! Browser-only design preview backed by localStorage. Database and import operations need the desktop runtime.
use crate::api::{local_storage_get, local_storage_set, now_iso};
use crate::types::{
    CanvasGroup, Diagram, DiagramEdit, DiagramEntry, DiagramFile, DocEntry, DocFile, Editable,
    LayoutBackup, Position, Project, RepoListing, Source,
};
use serde_json::Value;
use std::collections::HashMap;

const KEY: &str = "schema-atlas-browser-preview-v1";
const SAMPLE: &str = include_str!("sample.json");

fn load() -> Vec<Project> {
    local_storage_get(KEY)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save(projects: &[Project]) {
    if let Ok(raw) = serde_json::to_string(projects) {
        local_storage_set(KEY, &raw);
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn backup(source: &Source) -> LayoutBackup {
    LayoutBackup {
        positions: source.positions.clone(),
        groups: source.groups().to_vec(),
    }
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

const ATLAS_KEY: &str = "schema-atlas-browser-preview-atlas-v2";
const SAMPLE_DIAGRAM: &str = include_str!("sample-diagram.json");

/// Per project: diagram path → diagram, and slug → Markdown. The preview cannot write Mermaid,
/// so a diagram's "file text" here is its model as JSON; undo restores that.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Atlas {
    #[serde(default)]
    toc: Option<schematlas_ui::toc::Toc>,
    diagrams: std::collections::BTreeMap<String, Diagram>,
    docs: std::collections::BTreeMap<String, String>,
}
fn load_atlas() -> HashMap<String, Atlas> {
    local_storage_get(ATLAS_KEY)
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}
fn save_atlas(atlas: &HashMap<String, Atlas>) {
    if let Ok(raw) = serde_json::to_string(atlas) {
        local_storage_set(ATLAS_KEY, &raw);
    }
}
/// Content hash for compare-and-swap, like the backend's (any stable digest works here).
fn digest(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
fn diagram_text(diagram: &Diagram) -> String {
    serde_json::to_string_pretty(diagram).unwrap_or_default()
}
fn diagram_title(path: &str, diagram: &Diagram) -> String {
    diagram
        .annotation
        .title
        .clone()
        .or_else(|| diagram.title.clone())
        .unwrap_or_else(|| {
            let name = path.rsplit('/').next().unwrap_or(path);
            name.split('.')
                .next()
                .unwrap_or(name)
                .replace(['-', '_'], " ")
        })
}
fn diagram_file(path: &str, diagram: &Diagram) -> DiagramFile {
    let text = diagram_text(diagram);
    DiagramFile {
        id: path.to_string(),
        path: path.to_string(),
        part: None,
        format: "mermaid".into(),
        title: diagram_title(path, diagram),
        hash: digest(&text),
        editable: Editable::Full,
        source: text.clone(),
        text,
        diagram: diagram.clone(),
        schema: None,
    }
}
fn doc_title(markdown: &str, slug: &str) -> String {
    markdown
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| slug.to_string())
}
fn doc_path(slug: &str) -> String {
    if slug.ends_with(".md") || slug.ends_with(".markdown") {
        slug.into()
    } else {
        format!(".schematlas/docs/{slug}.md")
    }
}
fn new_diagram_path(atlas: &Atlas, title: &str) -> String {
    let mut slug: String = title
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "diagram" } else { slug };
    let mut path = format!(".schematlas/diagrams/{slug}.mmd");
    let mut n = 2;
    while atlas.diagrams.contains_key(&path) {
        path = format!(".schematlas/diagrams/{slug}-{n}.mmd");
        n += 1;
    }
    path
}
fn atlas_command(command: &str, args: &Value, project_id: &str) -> Option<Result<Value, String>> {
    if !matches!(
        command,
        "list_repo"
            | "create_toc_doc"
            | "delete_toc_doc"
            | "get_toc"
            | "edit_toc"
            | "get_toc_doc"
            | "save_toc_doc"
            | "get_diagram"
            | "edit_diagram"
            | "restore_diagram"
            | "reset_diagram_layout"
            | "create_diagram"
            | "delete_diagram"
            | "get_doc"
            | "save_doc"
            | "delete_doc"
    ) {
        return None;
    }
    let mut all = load_atlas();
    let atlas = all.entry(project_id.to_string()).or_default();
    let slug = text(args, "slug");
    let id = text(args, "id");
    let base = args
        .get("baseHash")
        .and_then(Value::as_str)
        .map(str::to_string);
    let stale = || Err("This diagram changed since it was read. Reload it and try again.".into());
    let result = (|| -> Result<Value, String> {
        match command {
            "create_toc_doc" => {
                use schematlas_ui::toc::{self, Edit, Entry, Snapshot};
                let snapshot: Snapshot = serde_json::from_value(
                    atlas_command("get_toc", args, project_id).ok_or("TOC unavailable")??,
                )
                .map_err(|e| e.to_string())?;
                if snapshot.hash != base {
                    return Err(
                        "toc.yaml changed since it was read. Reload it and try again.".into(),
                    );
                }
                let mut toc = snapshot.toc;
                let parent = args
                    .get("parent")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let title = text(args, "title");
                let base_path = toc::document_path(&toc, parent.as_deref(), &title)?;
                let mut path = base_path.clone();
                let mut suffix = 2;
                while atlas.docs.keys().any(|key| doc_path(key) == path) {
                    path = format!("{}-{suffix}.md", base_path.trim_end_matches(".md"));
                    suffix += 1;
                }
                toc.apply(Edit::Add {
                    entry: Entry::file(title.trim().into(), path.clone()),
                    parent,
                })?;
                let markdown = format!("# {}\n\n", title.trim());
                atlas.docs.insert(path.clone(), markdown.clone());
                atlas.toc = Some(toc.clone());
                Ok(
                    serde_json::json!({ "doc": DocFile { slug: path, hash: digest(&markdown), title, markdown }, "toc": Snapshot { root: Some("Browser preview".into()), hash: Some(digest(&toc::to_yaml(&toc))), toc, ..Snapshot::default() } }),
                )
            }
            "delete_toc_doc" => {
                let path = text(args, "path");
                let key = atlas
                    .docs
                    .keys()
                    .find(|key| doc_path(key) == path)
                    .cloned()
                    .ok_or("Document not found.")?;
                if atlas.docs.get(&key).map(|m| digest(m)) != base {
                    return Err("Document changed since it was read.".into());
                }
                atlas.docs.remove(&key);
                Ok(Value::Null)
            }
            "get_toc" | "edit_toc" => {
                use schematlas_ui::toc::{self, Entry, Snapshot};
                let project = load()
                    .into_iter()
                    .find(|p| p.id == project_id)
                    .ok_or("Project not found.")?;
                let hash = atlas.toc.as_ref().map(|t| digest(&toc::to_yaml(t)));
                let migrated = || {
                    let mut items = atlas
                        .diagrams
                        .iter()
                        .map(|(path, d)| {
                            let mut e = Entry::file(diagram_title(path, d), path.clone());
                            e.kind = Some("diagram".into());
                            (format!("diagram:{path}"), "diagrams".into(), e)
                        })
                        .chain(atlas.docs.iter().map(|(slug, md)| {
                            (
                                format!("doc:{slug}"),
                                "docs".into(),
                                Entry::file(doc_title(md, slug), doc_path(slug)),
                            )
                        }))
                        .collect::<Vec<_>>();
                    items.extend(project.sources.iter().map(|s| {
                        (
                            format!("source:{}", s.id),
                            if s.is_database() { "databases" } else { "apis" }.into(),
                            Entry::source(s.name.clone(), s.name.clone(), s.kind.clone()),
                        )
                    }));
                    toc::migrate(&project.explorer, &items)
                };
                let mut toc = atlas.toc.clone().unwrap_or_else(migrated);
                toc.validate()?;
                if command == "edit_toc" {
                    if hash != base {
                        return Err(
                            "toc.yaml changed since it was read. Reload it and try again.".into(),
                        );
                    }
                    if let Some(edit) = args.get("edit").filter(|e| !e.is_null()) {
                        toc.apply(
                            serde_json::from_value(edit.clone()).map_err(|e| e.to_string())?,
                        )?;
                    }
                    atlas.toc = Some(toc.clone());
                }
                let mut issues = std::collections::BTreeMap::new();
                for row in toc.rows() {
                    if let Some(file) = row.entry.file {
                        if !atlas.docs.keys().any(|slug| doc_path(slug) == file)
                            && !atlas.diagrams.contains_key(&file)
                        {
                            issues.insert(row.id, format!("Missing documentation file: {file}"));
                        }
                    }
                }
                to_value(&Snapshot {
                    root: Some("Browser preview".into()),
                    hash: atlas.toc.as_ref().map(|t| digest(&toc::to_yaml(t))),
                    toc,
                    issues,
                    hashes: atlas
                        .docs
                        .iter()
                        .map(|(key, md)| (doc_path(key), digest(md)))
                        .collect(),
                    error: None,
                })
            }
            "get_toc_doc" | "save_toc_doc" => {
                let path = text(args, "path");
                schematlas_ui::toc::valid_path(&path)?;
                let key = atlas
                    .docs
                    .keys()
                    .find(|key| doc_path(key) == path)
                    .cloned()
                    .unwrap_or_else(|| path.clone());
                if command == "save_toc_doc" {
                    if atlas.docs.get(&key).map(|m| digest(m)) != base {
                        return Err(
                            "This document changed since it was read. Reload it and try again."
                                .into(),
                        );
                    }
                    atlas.docs.insert(key.clone(), text(args, "markdown"));
                }
                let markdown = atlas.docs.get(&key).ok_or("Document not found.")?;
                to_value(&DocFile {
                    slug: path.clone(),
                    hash: digest(markdown),
                    title: doc_title(markdown, &path),
                    markdown: markdown.clone(),
                })
            }
            "list_repo" => {
                let diagrams: Vec<DiagramEntry> = atlas
                    .diagrams
                    .iter()
                    .map(|(path, d)| DiagramEntry {
                        id: path.clone(),
                        path: path.clone(),
                        part: None,
                        format: "mermaid".into(),
                        kind: d.kind.clone(),
                        title: diagram_title(path, d),
                        nodes: d.nodes.len(),
                        edges: d.edges.len(),
                        hash: digest(&diagram_text(d)),
                        editable: Editable::Full,
                        error: None,
                    })
                    .collect();
                let docs: Vec<DocEntry> = atlas
                    .docs
                    .iter()
                    .map(|(slug, md)| DocEntry {
                        slug: slug.clone(),
                        path: doc_path(slug),
                        title: doc_title(md, slug),
                        hash: digest(md),
                    })
                    .collect();
                to_value(&RepoListing {
                    root: Some("Browser preview".into()),
                    is_git: false,
                    diagrams,
                    docs,
                })
            }
            "get_diagram" => {
                let diagram = atlas.diagrams.get(&id).ok_or("Diagram not found.")?;
                to_value(&diagram_file(&id, diagram))
            }
            "edit_diagram" => {
                let diagram = atlas.diagrams.get(&id).ok_or("Diagram not found.")?;
                if base.as_deref() != Some(digest(&diagram_text(diagram)).as_str()) {
                    return stale();
                }
                let edits: Vec<DiagramEdit> =
                    serde_json::from_value(args.get("edits").cloned().unwrap_or(Value::Null))
                        .map_err(|e| e.to_string())?;
                let next = edits.iter().try_fold(diagram.clone(), |d, edit| {
                    crate::diagram_layout::apply_edit(&d, edit)
                })?;
                atlas.diagrams.insert(id.clone(), next.clone());
                to_value(&diagram_file(&id, &next))
            }
            "reset_diagram_layout" => {
                let mut diagram = atlas.diagrams.get(&id).ok_or("Diagram not found.")?.clone();
                diagram.layout.clear();
                atlas.diagrams.insert(id.clone(), diagram.clone());
                to_value(&diagram_file(&id, &diagram))
            }
            "restore_diagram" => {
                let diagram = atlas.diagrams.get(&id).ok_or("Diagram not found.")?;
                if base.as_deref() != Some(digest(&diagram_text(diagram)).as_str()) {
                    return stale();
                }
                let restored: Diagram =
                    serde_json::from_str(&text(args, "text")).map_err(|e| e.to_string())?;
                atlas.diagrams.insert(id.clone(), restored.clone());
                to_value(&diagram_file(&id, &restored))
            }
            "create_diagram" => {
                let title = text(args, "title");
                let source = text(args, "text");
                // Only a blank flowchart can be made without the desktop app's Mermaid reader.
                if source.lines().filter(|l| !l.trim().is_empty()).count() > 4 {
                    return Err("Reading pasted Mermaid needs the desktop app.".into());
                }
                let path = new_diagram_path(atlas, &title);
                let diagram = Diagram {
                    kind: "flowchart".into(),
                    direction: Some("LR".into()),
                    title: Some(title.trim().to_string()).filter(|t| !t.is_empty()),
                    ..Diagram::default()
                };
                atlas.diagrams.insert(path.clone(), diagram.clone());
                to_value(&diagram_file(&path, &diagram))
            }
            "delete_diagram" => {
                atlas.diagrams.remove(&id);
                Ok(Value::Null)
            }
            "get_doc" => {
                let markdown = atlas.docs.get(&slug).ok_or("Doc not found.")?;
                to_value(&DocFile {
                    slug: slug.clone(),
                    hash: digest(markdown),
                    title: doc_title(markdown, &slug),
                    markdown: markdown.clone(),
                })
            }
            "save_doc" => {
                let markdown = text(args, "markdown");
                let current = atlas.docs.get(&slug).map(|m| digest(m));
                if current != base {
                    return Err(
                        "This doc changed since it was read. Reload it and try again.".into(),
                    );
                }
                atlas.docs.insert(slug.clone(), markdown.clone());
                to_value(&DocFile {
                    hash: digest(&markdown),
                    title: doc_title(&markdown, &slug),
                    slug: slug.clone(),
                    markdown,
                })
            }
            _ => {
                atlas.docs.remove(&slug);
                Ok(Value::Null)
            }
        }
    })();
    if result.is_ok() {
        save_atlas(&all);
    }
    Some(result)
}

/// The demo project's atlas: one architecture map and an overview doc.
fn seed_demo_atlas(project_id: &str) {
    let mut all = load_atlas();
    let atlas = all.entry(project_id.to_string()).or_default();
    if let Ok(diagram) = serde_json::from_str::<Diagram>(SAMPLE_DIAGRAM) {
        atlas
            .diagrams
            .insert("docs/architecture.mmd".into(), diagram);
    }
    atlas.docs.insert(
        "overview".into(),
        "# Northstar Commerce\n\nThe storefront calls the **Orders API**, which writes to the commerce database and hands payments to the PSP.\n\n[System map](schematlas:diagram/docs/architecture.mmd)\n\n- Architecture map: [Architecture](schematlas:diagram/docs/architecture.mmd)\n- Checkout entry point: `src/api/checkout.rs:18-64`\n".into(),
    );
    save_atlas(&all);
}

pub fn preview(command: &str, args: Value) -> Result<Value, String> {
    let mut projects = load();
    if let Some(result) = atlas_command(command, &args, &text(&args, "projectId")) {
        return result;
    }
    let project_id = text(&args, "projectId");
    let index = projects.iter().position(|p| p.id == project_id);
    let result = match command {
        "list_projects" => return to_value(&projects),
        "create_demo" => {
            let mut demo: Project = serde_json::from_str(SAMPLE).map_err(|e| e.to_string())?;
            demo.id = uuid::Uuid::new_v4().to_string();
            demo.created_at = now_iso();
            demo.updated_at = demo.created_at.clone();
            for source in &mut demo.sources {
                source.id = uuid::Uuid::new_v4().to_string();
            }
            projects.push(demo.clone());
            seed_demo_atlas(&demo.id);
            to_value(&demo)?
        }
        "create_project" => {
            let project = Project {
                explorer: Default::default(),
                id: uuid::Uuid::new_v4().to_string(),
                name: text(&args, "name").trim().to_string(),
                description: text(&args, "description"),
                created_at: now_iso(),
                updated_at: now_iso(),
                sources: vec![],
                working_directory: None,
                sync: None,
            };
            projects.push(project.clone());
            to_value(&project)?
        }
        "rename_project" => {
            let project = index
                .map(|i| &mut projects[i])
                .ok_or("Project not found.")?;
            project.name = text(&args, "name");
            project.description = text(&args, "description");
            to_value(project)?
        }
        "delete_project" => {
            projects.retain(|p| p.id != project_id);
            save(&projects);
            return Ok(Value::Null);
        }
        "remove_source" => {
            let source_id = text(&args, "sourceId");
            let project = index
                .map(|i| &mut projects[i])
                .ok_or("Project not found.")?;
            project.sources.retain(|s| s.id != source_id);
            to_value(project)?
        }
        "save_positions" => {
            let source_id = text(&args, "sourceId");
            let positions: HashMap<String, Position> =
                serde_json::from_value(args.get("positions").cloned().unwrap_or(Value::Null))
                    .map_err(|e| e.to_string())?;
            let project = index.map(|i| &mut projects[i]).ok_or("Source not found.")?;
            let source = project
                .sources
                .iter_mut()
                .find(|s| s.id == source_id)
                .ok_or("Source not found.")?;
            source.layout_backup = Some(backup(source));
            source.positions.extend(positions);
            to_value(project)?
        }
        "undo_canvas" | "remove_canvas_group" => {
            let source_id = text(&args, "sourceId");
            let project = index.map(|i| &mut projects[i]).ok_or("Source not found.")?;
            let source = project
                .sources
                .iter_mut()
                .find(|s| s.id == source_id)
                .ok_or("Source not found.")?;
            if command == "undo_canvas" {
                let restored = source
                    .layout_backup
                    .take()
                    .ok_or("No canvas edit to undo.")?;
                source.positions = restored.positions;
                source.groups = Some(restored.groups);
            } else {
                let group_id = text(&args, "groupId");
                source.layout_backup = Some(backup(source));
                source.groups = Some(
                    source
                        .groups()
                        .iter()
                        .filter(|g| g.id != group_id)
                        .cloned()
                        .collect(),
                );
            }
            to_value(project)?
        }
        "set_canvas_group" => {
            let input = args.get("args").cloned().unwrap_or(Value::Null);
            let source_id = text(&input, "source_id");
            let group_id = input
                .get("group_id")
                .and_then(Value::as_str)
                .map(str::to_string);
            let name = text(&input, "name").trim().to_string();
            let node_ids: Vec<String> = input
                .get("node_ids")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            let project = index.map(|i| &mut projects[i]).ok_or("Source not found.")?;
            let source = project
                .sources
                .iter_mut()
                .find(|s| s.id == source_id)
                .ok_or("Source not found.")?;
            let previous = source
                .groups()
                .iter()
                .find(|g| Some(&g.id) == group_id.as_ref())
                .cloned();
            if group_id.is_some() && previous.is_none() {
                return Err("Group not found.".into());
            }
            let color = input
                .get("color")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| previous.as_ref().and_then(|g| g.color.clone()))
                .unwrap_or_else(|| crate::layout::DEFAULT_GROUP_COLOR.to_string());
            let mut unique = node_ids.clone();
            unique.sort();
            unique.dedup();
            let valid_color = color.len() == 7
                && color.starts_with('#')
                && color[1..].chars().all(|c| c.is_ascii_hexdigit());
            if name.is_empty()
                || name.len() > 80
                || !valid_color
                || node_ids.is_empty()
                || unique.len() != node_ids.len()
                || node_ids
                    .iter()
                    .any(|id| !source.graph.entities.iter().any(|e| &e.id == id))
            {
                return Err("Choose a name, color, and valid group members.".into());
            }
            if previous.is_none() && source.groups().len() >= 100 {
                return Err("Use at most 100 groups per source.".into());
            }
            source.layout_backup = Some(backup(source));
            let group = CanvasGroup {
                id: previous
                    .as_ref()
                    .map(|g| g.id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                name,
                node_ids,
                color: Some(color.to_lowercase()),
            };
            let mut groups = source.groups().to_vec();
            if previous.is_some() {
                for g in &mut groups {
                    if g.id == group.id {
                        *g = group.clone();
                    }
                }
            } else {
                groups.push(group);
            }
            source.groups = Some(groups);
            to_value(project)?
        }
        "import_mermaid" => return Err("Mermaid import runs in the desktop app.".into()),
        command if command.starts_with("sync_") => {
            return Err(
                "Team sync runs in the desktop app, through your repository's git remote.".into(),
            )
        }
        _ => {
            return Err(
                "Open the Schematlas desktop app to connect databases or import OpenAPI files."
                    .into(),
            )
        }
    };
    save(&projects);
    Ok(result)
}
