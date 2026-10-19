//! Diagrams as the repository already writes them — Mermaid files and blocks, draw.io pages, DBML
//! schemas, Excalidraw scenes — read into one model and edited in place through
//! `schematlas-parser`. Positions that a format cannot store (Mermaid, DBML) live beside it in
//! `.schematlas/layout/`.
use crate::error::{AppError, Result};
use crate::repo::{Format, Repo, ATLAS_DIR};
use schematlas_parser::annotate::Target;
use schematlas_parser::model::{self, Diagram, Edge, Edit, Group, Head, Node, Rect, Stroke};
use schematlas_parser::{dbml, drawio, excalidraw, markdown, mermaid, sketch};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `path` for a whole-file diagram, `path#n` for the n-th Mermaid block of a Markdown file or the
/// n-th page of a draw.io file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DiagramId {
    pub path: String,
    pub part: Option<usize>,
}
impl DiagramId {
    pub fn parse(id: &str) -> Result<DiagramId> {
        let (path, part) = match id.rsplit_once('#') {
            Some((path, n)) if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() => (
                path,
                Some(
                    n.parse()
                        .map_err(|_| AppError::Validation("Bad diagram id.".into()))?,
                ),
            ),
            _ => (id, None),
        };
        crate::repo::valid_relative(path)?;
        Ok(DiagramId {
            path: path.to_owned(),
            part,
        })
    }
    pub fn text(&self) -> String {
        match self.part {
            Some(n) => format!("{}#{n}", self.path),
            None => self.path.clone(),
        }
    }
    fn format(&self) -> Result<Format> {
        Format::of(&self.path)
            .ok_or_else(|| AppError::Validation(format!("`{}` is not a diagram file.", self.path)))
    }
}

/// What can be changed from Schematlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Editable {
    /// Nodes, edges, labels, layout and annotations.
    Full,
    /// Annotations and layout only (class, ER, state diagrams; DBML).
    Annotations,
    /// Nothing (exported `.drawio.svg` images).
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramEntry {
    pub id: String,
    pub path: String,
    pub part: Option<usize>,
    pub format: Format,
    pub kind: String,
    pub title: String,
    pub nodes: usize,
    pub edges: usize,
    pub hash: String,
    pub editable: Editable,
    /// Files that fail to parse are listed with the reason instead of disappearing.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramFile {
    pub id: String,
    pub path: String,
    pub part: Option<usize>,
    pub format: Format,
    pub title: String,
    /// Hash of the whole file, the base for the next edit.
    pub hash: String,
    pub editable: Editable,
    /// The diagram's own source: the file, a Markdown block, or the draw.io file.
    pub source: String,
    /// The whole file, so an undo can restore it exactly.
    pub text: String,
    pub diagram: Diagram,
    /// DBML only: tables, typed columns and column-level references, drawn by the schema canvas.
    /// Entity ids are the diagram's node ids (`schema.table`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<crate::domain::Graph>,
}

fn stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = name
        .split('.')
        .next()
        .unwrap_or(name)
        .replace(['-', '_'], " ");
    // `ai-platform-request-flow` reads as "Ai platform request flow".
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => name,
    }
}
fn title_of(d: &Diagram, fallback: String) -> String {
    d.annotation
        .title
        .clone()
        .or_else(|| d.title.clone())
        .unwrap_or(fallback)
}

/// A DBML schema as a diagram: tables with their columns, relationships as edges.
pub fn schema_diagram(schema: &dbml::Schema) -> Diagram {
    let mut d = Diagram {
        kind: "dbml".into(),
        annotation: schema.annotation.clone(),
        ..Diagram::default()
    };
    d.title = schema.project.as_ref().map(|p| p.name.clone());
    for t in &schema.tables {
        d.nodes.push(Node {
            id: t.qualified(),
            label: if t.schema == dbml::DEFAULT_SCHEMA {
                t.name.clone()
            } else {
                t.qualified()
            },
            shape: Some("table".into()),
            details: t
                .columns
                .iter()
                .map(|c| {
                    let mut line = format!("{} {}", c.name, c.data_type);
                    if c.primary_key {
                        line.push_str(" PK");
                    }
                    if c.not_null && !c.primary_key {
                        line.push_str(" NOT NULL");
                    }
                    line
                })
                .collect(),
            annotation: t.annotation.clone(),
            ..Node::default()
        });
    }
    for r in &schema.refs {
        let (tail, head) = match r.cardinality {
            dbml::Cardinality::ManyToOne => ("n", "1"),
            dbml::Cardinality::OneToOne => ("1", "1"),
            dbml::Cardinality::ManyToMany => ("n", "n"),
        };
        d.edges.push(Edge {
            from: r.from.qualified_table(),
            to: r.to.qualified_table(),
            label: Some(format!(
                "{} → {}",
                r.from.columns.join(", "),
                r.to.columns.join(", ")
            )),
            head: Head::None,
            stroke: Stroke::Normal,
            tail_label: Some(tail.into()),
            head_label: Some(head.into()),
            ..Edge::default()
        });
    }
    for g in &schema.groups {
        d.groups.push(Group {
            id: format!("group:{}", g.name),
            title: g.name.clone(),
            nodes: g.tables.clone(),
            ..Group::default()
        });
    }
    d
}

/// Every diagram a file holds, read without layouts (listing is about names and sizes).
pub fn entries(path: &str, text: &str, hash: &str) -> Vec<DiagramEntry> {
    let Some(format) = Format::of(path) else {
        return vec![];
    };
    let entry = |part: Option<usize>,
                 kind: &str,
                 title: String,
                 d: Option<&Diagram>,
                 editable: Editable,
                 error: Option<String>| DiagramEntry {
        id: DiagramId {
            path: path.to_owned(),
            part,
        }
        .text(),
        path: path.to_owned(),
        part,
        format,
        kind: kind.to_owned(),
        title,
        nodes: d.map_or(0, |d| d.nodes.len()),
        edges: d.map_or(0, |d| d.edges.len()),
        hash: hash.to_owned(),
        editable,
        error,
    };
    match format {
        Format::Mermaid => match mermaid::parse(text) {
            Ok(doc) => {
                let editable = if doc.diagram.kind == "flowchart" {
                    Editable::Full
                } else {
                    Editable::Annotations
                };
                vec![entry(
                    None,
                    &doc.diagram.kind.clone(),
                    title_of(&doc.diagram, stem(path)),
                    Some(&doc.diagram),
                    editable,
                    None,
                )]
            }
            Err(e) => vec![entry(
                None,
                "mermaid",
                stem(path),
                None,
                Editable::None,
                Some(e.to_string()),
            )],
        },
        Format::Markdown => markdown::mermaid_blocks(text)
            .into_iter()
            .filter_map(|block| {
                let title = block
                    .heading
                    .clone()
                    .unwrap_or_else(|| format!("{} #{}", stem(path), block.index + 1));
                match mermaid::parse(&block.source(text)) {
                    Ok(doc) => {
                        let editable = if doc.diagram.kind == "flowchart" {
                            Editable::Full
                        } else {
                            Editable::Annotations
                        };
                        Some(entry(
                            Some(block.index),
                            &doc.diagram.kind.clone(),
                            title_of(&doc.diagram, title),
                            Some(&doc.diagram),
                            editable,
                            None,
                        ))
                    }
                    // Sequence diagrams and friends in a README are not graphs; leave them out.
                    Err(schematlas_parser::Error::Unsupported(_)) => None,
                    Err(e) => Some(entry(
                        Some(block.index),
                        "mermaid",
                        title,
                        None,
                        Editable::None,
                        Some(e.to_string()),
                    )),
                }
            })
            .collect(),
        Format::Drawio => {
            let svg = path.to_ascii_lowercase().ends_with(".svg");
            match drawio::parse(text) {
                Ok(pages) => {
                    let many = pages.len() > 1;
                    pages
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let title = if many {
                                format!("{} — {}", stem(path), p.name)
                            } else {
                                title_of(&p.diagram, stem(path))
                            };
                            entry(
                                Some(i),
                                "drawio",
                                title,
                                Some(&p.diagram),
                                if svg { Editable::None } else { Editable::Full },
                                None,
                            )
                        })
                        .collect()
                }
                Err(e) => vec![entry(
                    Some(0),
                    "drawio",
                    stem(path),
                    None,
                    Editable::None,
                    Some(e.to_string()),
                )],
            }
        }
        Format::Excalidraw => match excalidraw::parse(text) {
            Ok(d) => vec![entry(
                None,
                excalidraw::KIND,
                title_of(&d, stem(path)),
                Some(&d),
                Editable::Annotations,
                None,
            )],
            Err(e) => vec![entry(
                None,
                excalidraw::KIND,
                stem(path),
                None,
                Editable::None,
                Some(e.to_string()),
            )],
        },
        Format::Dbml => match dbml::parse(text) {
            Ok(schema) => {
                let d = schema_diagram(&schema);
                vec![entry(
                    None,
                    "dbml",
                    title_of(&d, stem(path)),
                    Some(&d),
                    Editable::Annotations,
                    None,
                )]
            }
            Err(e) => vec![entry(
                None,
                "dbml",
                stem(path),
                None,
                Editable::None,
                Some(e.to_string()),
            )],
        },
    }
}

// ---------- Layouts beside formats without positions ----------

#[derive(Default, Serialize, Deserialize)]
struct Layout {
    #[serde(default)]
    positions: BTreeMap<String, Rect>,
}
fn layout_path(id: &DiagramId) -> String {
    let mut path = format!("{ATLAS_DIR}/layout/{}", id.path);
    if let Some(part) = id.part {
        path.push_str(&format!(".{part}"));
    }
    path.push_str(".json");
    path
}
fn read_layout(repo: &Repo, id: &DiagramId) -> (Layout, Option<String>) {
    match repo.read(&layout_path(id)) {
        Ok(Some((text, hash))) => (serde_json::from_str(&text).unwrap_or_default(), Some(hash)),
        _ => (Layout::default(), None),
    }
}

// ---------- Loading and editing ----------

fn read_file(repo: &Repo, path: &str) -> Result<(String, String)> {
    repo.read(path)?.ok_or(AppError::NotFound)
}

/// The diagram, its source, and (for Mermaid and DBML) its saved layout.
pub fn load(repo: &Repo, id: &DiagramId) -> Result<DiagramFile> {
    let format = id.format()?;
    let (text, hash) = read_file(repo, &id.path)?;
    let parse_error = |e: schematlas_parser::Error| AppError::Validation(e.to_string());
    // DBML also comes as tables with typed columns, for the schema renderer.
    let mut schema = None;
    let (diagram, source, title) = match format {
        Format::Mermaid => {
            let d = mermaid::parse(&text).map_err(parse_error)?.diagram;
            let title = title_of(&d, stem(&id.path));
            (d, text.clone(), title)
        }
        Format::Markdown => {
            let index = id.part.ok_or_else(|| {
                AppError::Validation("Name the Mermaid block: `file.md#0`.".into())
            })?;
            let block = markdown::mermaid_blocks(&text)
                .into_iter()
                .nth(index)
                .ok_or(AppError::NotFound)?;
            let source = block.source(&text);
            let d = mermaid::parse(&source).map_err(parse_error)?.diagram;
            let title = title_of(&d, block.heading.clone().unwrap_or_else(|| stem(&id.path)));
            (d, source, title)
        }
        Format::Drawio => {
            let index = id.part.unwrap_or(0);
            let mut pages = drawio::parse(&text).map_err(parse_error)?;
            if index >= pages.len() {
                return Err(AppError::NotFound);
            }
            let page = pages.swap_remove(index);
            let title = page.name.clone();
            (page.diagram, text.clone(), title)
        }
        Format::Excalidraw => {
            let d = excalidraw::parse(&text).map_err(parse_error)?;
            let title = title_of(&d, stem(&id.path));
            (d, text.clone(), title)
        }
        Format::Dbml => {
            let parsed = dbml::parse(&text).map_err(parse_error)?;
            schema = Some(crate::dbml_export::graph(&parsed));
            let d = schema_diagram(&parsed);
            let title = title_of(&d, stem(&id.path));
            (d, text.clone(), title)
        }
    };
    let mut diagram = diagram;
    if !format.keeps_positions() {
        let (layout, _) = read_layout(repo, id);
        let known: Vec<&str> = diagram
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .chain(diagram.groups.iter().map(|g| g.id.as_str()))
            .collect();
        diagram.layout = layout
            .positions
            .into_iter()
            .filter(|(k, _)| known.contains(&k.as_str()))
            .collect();
    }
    if format == Format::Excalidraw {
        follow_annotations(repo, &mut diagram);
    }
    let editable = match format {
        Format::Drawio if id.path.to_ascii_lowercase().ends_with(".svg") => Editable::None,
        Format::Drawio => Editable::Full,
        // The drawing is edited in the Excalidraw editor, which writes the whole scene.
        Format::Dbml | Format::Excalidraw => Editable::Annotations,
        _ if diagram.kind == "flowchart" => Editable::Full,
        _ => Editable::Annotations,
    };
    Ok(DiagramFile {
        id: id.text(),
        path: id.path.clone(),
        part: id.part,
        format,
        title,
        hash,
        editable,
        source,
        text,
        diagram,
        schema,
    })
}

/// The result of edits before anything is written: the new file text and diagram, and the
/// layout to save beside the file if positions moved.
pub struct Planned {
    pub text: String,
    pub diagram: Diagram,
    layout: Option<Layout>,
}

/// Work out what `edits` would do to the diagram, without writing anything.
pub fn plan(repo: &Repo, id: &DiagramId, current: &DiagramFile, edits: &[Edit]) -> Result<Planned> {
    let format = id.format()?;
    if current.editable == Editable::None {
        return Err(AppError::Validation(
            "This file is read-only here; edit the file it was exported from.".into(),
        ));
    }
    let refuse = |e: schematlas_parser::Error| AppError::Validation(e.to_string());
    let mut text = current.text.clone();
    let mut diagram = current.diagram.clone();
    let (mut layout, _) = read_layout(repo, id);
    let mut layout_changed = false;
    for edit in edits {
        if let Edit::Move { id: target, x, y } = edit {
            if format == Format::Excalidraw {
                return Err(AppError::Validation(
                    "Excalidraw scenes are arranged in the Excalidraw editor.".into(),
                ));
            }
            if format != Format::Drawio {
                // Positions this format cannot hold are kept beside it.
                if !diagram.nodes.iter().any(|n| &n.id == target)
                    && !diagram.groups.iter().any(|g| &g.id == target)
                {
                    return Err(AppError::Validation(format!("No node `{target}`.")));
                }
                if !x.is_finite() || !y.is_finite() {
                    return Err(AppError::Validation("Positions must be finite.".into()));
                }
                let size = layout.positions.get(target).copied().unwrap_or(Rect {
                    x: 0.0,
                    y: 0.0,
                    width: model::NEW_NODE_SIZE.0,
                    height: model::NEW_NODE_SIZE.1,
                });
                layout.positions.insert(
                    target.clone(),
                    Rect {
                        x: x.round(),
                        y: y.round(),
                        ..size
                    },
                );
                diagram.layout.insert(
                    target.clone(),
                    Rect {
                        x: x.round(),
                        y: y.round(),
                        ..size
                    },
                );
                layout_changed = true;
                continue;
            }
        }
        if current.editable == Editable::Annotations && !matches!(edit, Edit::Annotate { .. }) {
            return Err(AppError::Validation(format!(
                "{} diagrams take annotations and layout here, not structural edits.",
                diagram.kind
            )));
        }
        text = match format {
            Format::Mermaid => mermaid::apply(&text, edit).map_err(refuse)?,
            Format::Markdown => {
                mermaid::apply_in_markdown(&text, id.part.unwrap_or(0), edit).map_err(refuse)?
            }
            Format::Drawio => drawio::apply(&text, id.part.unwrap_or(0), edit).map_err(refuse)?,
            Format::Excalidraw => match edit {
                Edit::Annotate { target, annotation } => {
                    excalidraw::annotate(&text, target, annotation).map_err(refuse)?
                }
                _ => {
                    return Err(AppError::Validation(
                        "Excalidraw scenes are drawn in the Excalidraw editor; here they take annotations.".into(),
                    ))
                }
            },
            Format::Dbml => match edit {
                Edit::Annotate { target, annotation } => {
                    if matches!(target, Target::Edge(_)) {
                        return Err(AppError::Validation(
                            "DBML annotations attach to tables or the schema.".into(),
                        ));
                    }
                    dbml::annotate(&text, target, annotation).map_err(refuse)?
                }
                _ => {
                    return Err(AppError::Validation(
                        "DBML schemas take annotations and layout here.".into(),
                    ))
                }
            },
        };
        let layout_before = std::mem::take(&mut diagram.layout);
        diagram = model::apply(&diagram, edit).map_err(refuse)?;
        if format != Format::Drawio {
            diagram.layout = layout_before;
        }
    }
    let layout = layout_changed.then(|| {
        // Drop positions of nodes that no longer exist.
        layout.positions.retain(|k, _| {
            diagram.nodes.iter().any(|n| &n.id == k) || diagram.groups.iter().any(|g| &g.id == k)
        });
        layout
    });
    Ok(Planned {
        text,
        diagram,
        layout,
    })
}

/// Write a plan, if the file still has the content it was planned from.
pub fn commit(
    repo: &Repo,
    id: &DiagramId,
    current: &DiagramFile,
    planned: Planned,
) -> Result<DiagramFile> {
    if let Some(layout) = planned.layout {
        let json = serde_json::to_string_pretty(&layout)? + "\n";
        let (_, layout_hash) = read_layout(repo, id);
        repo.write(&layout_path(id), &json, layout_hash.as_deref())?;
    }
    if planned.text != current.text {
        repo.write(&id.path, &planned.text, Some(&current.hash))?;
    }
    load(repo, id)
}

/// Apply edits in order, then write the file once (compare-and-swap against `base`) and the
/// layout beside it if positions moved. Any refused edit leaves everything untouched.
pub fn edit(repo: &Repo, id: &DiagramId, edits: &[Edit], base: &str) -> Result<DiagramFile> {
    let current = load(repo, id)?;
    if current.hash != base {
        return Err(AppError::Validation(format!(
            "{} changed since it was read. Reload it and try again.",
            id.path
        )));
    }
    // A sketch's components are its source's: their notes and code links are written there.
    if let Some(source_id) = sketch_source(&current) {
        let source = load(repo, &source_id)?;
        let (theirs, ours): (Vec<Edit>, Vec<Edit>) = edits.iter().cloned().partition(|e| {
            matches!(e, Edit::Annotate { target: Target::Node(n), .. }
                if source.diagram.nodes.iter().any(|x| &x.id == n))
        });
        if !theirs.is_empty() {
            edit(repo, &source_id, &theirs, &source.hash)?;
        }
        if ours.is_empty() {
            return load(repo, id);
        }
        let planned = plan(repo, id, &current, &ours)?;
        return commit(repo, id, &current, planned);
    }
    let planned = plan(repo, id, &current, edits)?;
    commit(repo, id, &current, planned)
}

// ---------- Sketches that follow a diagram ----------

/// The diagram a sketch follows, if it follows one.
fn sketch_source(file: &DiagramFile) -> Option<DiagramId> {
    if file.format != Format::Excalidraw {
        return None;
    }
    let id = DiagramId::parse(sketch::source_of(&file.diagram)?).ok()?;
    (Format::of(&id.path) != Some(Format::Excalidraw)).then_some(id)
}
/// What a sketch and its source last agreed on, kept beside the sketch.
fn synced_path(id: &DiagramId) -> String {
    format!("{ATLAS_DIR}/layout/{}.sync.json", id.path)
}
fn read_synced(repo: &Repo, id: &DiagramId) -> (sketch::Synced, Option<String>) {
    match repo.read(&synced_path(id)) {
        Ok(Some((text, hash))) => (serde_json::from_str(&text).unwrap_or_default(), Some(hash)),
        _ => (sketch::Synced::default(), None),
    }
}
fn write_synced(repo: &Repo, id: &DiagramId, synced: &sketch::Synced) -> Result<()> {
    let (old, hash) = read_synced(repo, id);
    if hash.is_some() && &old == synced {
        return Ok(());
    }
    let json = serde_json::to_string_pretty(synced)? + "\n";
    repo.write(&synced_path(id), &json, hash.as_deref())?;
    Ok(())
}
/// Shapes linked to the source's nodes show the source's notes and code links.
fn follow_annotations(repo: &Repo, sketch_diagram: &mut Diagram) {
    let Some(source) = sketch::source_of(sketch_diagram)
        .and_then(|s| DiagramId::parse(s).ok())
        .filter(|s| Format::of(&s.path) != Some(Format::Excalidraw))
    else {
        return;
    };
    let Ok(source) = load(repo, &source) else {
        return;
    };
    for node in &mut sketch_diagram.nodes {
        if let Some(theirs) = source.diagram.nodes.iter().find(|n| n.id == node.id) {
            if !theirs.annotation.is_empty() {
                node.annotation = theirs.annotation.clone();
            }
        }
    }
}

/// A sketch of `source_id`, drawn at `layout` (the positions the app shows it at), kept in step
/// with it from now on.
pub fn create_sketch(
    repo: &Repo,
    source_id: &DiagramId,
    layout: &BTreeMap<String, Rect>,
) -> Result<DiagramFile> {
    let source = load(repo, source_id)?;
    if source.format == Format::Excalidraw {
        return Err(AppError::Validation("This is already a sketch.".into()));
    }
    let title = format!("{} (sketch)", source.title);
    let empty = sketch::new(&source_id.text(), Some(&title));
    let out = sketch::sync(
        &empty,
        &source.diagram,
        &sketch::Synced::default(),
        true,
        layout,
    )
    .map_err(|e| AppError::Validation(e.to_string()))?;
    let path = new_path(repo, &title, &empty);
    let file = create(repo, &path, &out.sketch)?;
    let id = DiagramId::parse(&file.id)?;
    write_synced(repo, &id, &out.synced)?;
    Ok(file)
}

/// Bring a sketch and its source in step: what was drawn in the sketch is written to the source
/// (flowcharts only), what changed in the source is redrawn in the sketch.
pub fn sync_sketch(repo: &Repo, id: &DiagramId) -> Result<DiagramFile> {
    let current = load(repo, id)?;
    let Some(source_id) = sketch_source(&current) else {
        return Ok(current);
    };
    let source = load(repo, &source_id)?;
    let (base, _) = read_synced(repo, id);
    let two_way = source.editable == Editable::Full;
    let out = sketch::sync(
        &current.text,
        &source.diagram,
        &base,
        two_way,
        &source.diagram.layout,
    )
    .map_err(|e| AppError::Validation(e.to_string()))?;
    if !out.edits.is_empty() {
        edit(repo, &source_id, &out.edits, &source.hash)?;
    }
    if out.sketch != current.text {
        repo.write(&id.path, &out.sketch, Some(&current.hash))?;
    }
    write_synced(repo, id, &out.synced)?;
    load(repo, id)
}

/// Forget the positions kept beside a Mermaid or DBML file, so it is laid out automatically.
pub fn reset_layout(repo: &Repo, id: &DiagramId) -> Result<DiagramFile> {
    if id.format()?.keeps_positions() {
        return Err(AppError::Validation(
            "This file keeps its positions in the file itself.".into(),
        ));
    }
    let path = layout_path(id);
    if let Some((_, hash)) = repo.read(&path)? {
        repo.delete(&path, &hash)?;
    }
    load(repo, id)
}

/// Put back an earlier text of the file (undo), if the file is still as last read.
pub fn restore(repo: &Repo, id: &DiagramId, text: &str, base: &str) -> Result<DiagramFile> {
    validate_text(&id.path, text)?;
    repo.write(&id.path, text, Some(base))?;
    let file = load(repo, id)?;
    if sketch_source(&file).is_some() {
        // The drawing is saved either way; a failed sync (its source moved meanwhile) is tried
        // again on the next save or when the source changes.
        return Ok(sync_sketch(repo, id).unwrap_or(file));
    }
    Ok(file)
}

fn validate_text(path: &str, text: &str) -> Result<()> {
    let refuse = |e: schematlas_parser::Error| AppError::Validation(e.to_string());
    match Format::of(path) {
        Some(Format::Mermaid) => mermaid::parse(text).map(|_| ()).map_err(refuse),
        Some(Format::Drawio) => drawio::parse(text).map(|_| ()).map_err(refuse),
        Some(Format::Dbml) => dbml::parse(text).map(|_| ()).map_err(refuse),
        Some(Format::Excalidraw) => excalidraw::parse(text).map(|_| ()).map_err(refuse),
        Some(Format::Markdown) => Ok(()),
        None => Err(AppError::Validation(format!(
            "`{path}` is not a diagram file."
        ))),
    }
}

/// A new diagram file. Mermaid by default: readable on GitHub and by every agent.
pub fn create(repo: &Repo, path: &str, text: &str) -> Result<DiagramFile> {
    if !matches!(
        Format::of(path),
        Some(Format::Mermaid | Format::Drawio | Format::Dbml | Format::Excalidraw)
    ) {
        return Err(AppError::Validation(
            "New diagrams are .mmd, .drawio, .dbml or .excalidraw files.".into(),
        ));
    }
    validate_text(path, text)?;
    repo.write(path, text, None)?;
    let id = DiagramId {
        path: path.to_owned(),
        part: Format::of(path).filter(|f| *f == Format::Drawio).map(|_| 0),
    };
    load(repo, &id)
}

/// A file path for a new diagram titled `title`, unused so far: an Excalidraw scene when `text`
/// is one, else Mermaid.
pub fn new_path(repo: &Repo, title: &str, text: &str) -> String {
    let extension = if excalidraw::parse(text).is_ok() {
        "excalidraw"
    } else {
        "mmd"
    };
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
    let mut path = format!(".schematlas/diagrams/{slug}.{extension}");
    let mut n = 2;
    while repo.read(&path).ok().flatten().is_some() {
        path = format!(".schematlas/diagrams/{slug}-{n}.{extension}");
        n += 1;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use schematlas_parser::model::{Annotation, CodeRef};

    fn repo() -> (tempfile::TempDir, Repo) {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo::open(dir.path().to_str().unwrap()).unwrap();
        (dir, repo)
    }

    #[test]
    fn ids_name_files_blocks_and_pages() {
        assert_eq!(
            DiagramId::parse("docs/a.md#2").unwrap(),
            DiagramId {
                path: "docs/a.md".into(),
                part: Some(2)
            }
        );
        assert_eq!(DiagramId::parse("docs/a#b.mmd").unwrap().part, None);
        assert!(DiagramId::parse("../a.mmd").is_err());
    }

    #[test]
    fn mermaid_files_are_edited_in_place_with_layout_beside_them() {
        let (dir, repo) = repo();
        let file = create(
            &repo,
            ".schematlas/diagrams/checkout.mmd",
            "flowchart LR\n  api[API] --> db[(DB)]\n",
        )
        .unwrap();
        assert_eq!(file.editable, Editable::Full);
        let id = DiagramId::parse(&file.id).unwrap();
        let edits = [
            Edit::SetLabel {
                id: "api".into(),
                label: "Orders API".into(),
            },
            Edit::Move {
                id: "api".into(),
                x: 10.4,
                y: 20.0,
            },
            Edit::Annotate {
                target: Target::Node("db".into()),
                annotation: Annotation {
                    code: vec![CodeRef::parse("src/db.rs:3")],
                    ..Annotation::default()
                },
            },
        ];
        let after = edit(&repo, &id, &edits, &file.hash).unwrap();
        let text =
            std::fs::read_to_string(dir.path().join(".schematlas/diagrams/checkout.mmd")).unwrap();
        assert!(text.contains("api[Orders API] --> db[(DB)]"), "{text}");
        assert!(
            text.contains("%% @schematlas node db code=src/db.rs:3"),
            "{text}"
        );
        assert!(
            !text.contains("10"),
            "positions are not written into Mermaid: {text}"
        );
        assert_eq!(after.diagram.layout["api"].x, 10.0);
        assert!(dir
            .path()
            .join(".schematlas/layout/.schematlas/diagrams/checkout.mmd.json")
            .is_file());
        // A stale base is refused and nothing changes.
        assert!(edit(
            &repo,
            &id,
            &[Edit::RemoveNode { id: "api".into() }],
            &file.hash
        )
        .is_err());
        // Undo puts the previous text back.
        let undone = restore(&repo, &id, &file.text, &after.hash).unwrap();
        assert_eq!(undone.text, file.text);
        // One bad edit in a batch leaves the file as it was.
        let before =
            std::fs::read_to_string(dir.path().join(".schematlas/diagrams/checkout.mmd")).unwrap();
        assert!(edit(
            &repo,
            &id,
            &[
                Edit::SetLabel {
                    id: "api".into(),
                    label: "X".into()
                },
                Edit::RemoveNode { id: "ghost".into() }
            ],
            &undone.hash
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".schematlas/diagrams/checkout.mmd")).unwrap(),
            before
        );
    }

    #[test]
    fn markdown_blocks_drawio_pages_and_dbml_schemas() {
        let (dir, repo) = repo();
        std::fs::write(dir.path().join("README.md"), "# Project\n\n## Architecture\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n```mermaid\nsequenceDiagram\n  A->>B: hi\n```\n\n```mermaid\ngantt\n  title Plan\n```\n").unwrap();
        let (text, hash) = repo.read("README.md").unwrap().unwrap();
        let listed = entries("README.md", &text, &hash);
        // Sequence diagrams are drawn; families Schematlas cannot draw are left out.
        assert_eq!(listed.len(), 2, "{listed:#?}");
        assert_eq!(listed[1].kind, "sequence");
        assert_eq!(
            (listed[0].id.as_str(), listed[0].title.as_str()),
            ("README.md#0", "Architecture")
        );
        let id = DiagramId::parse("README.md#0").unwrap();
        let file = load(&repo, &id).unwrap();
        let after = edit(
            &repo,
            &id,
            &[Edit::AddEdge {
                from: "b".into(),
                to: "a".into(),
                label: None,
            }],
            &file.hash,
        )
        .unwrap();
        assert_eq!(after.diagram.edges.len(), 2);
        assert!(after.text.starts_with("# Project\n\n## Architecture\n"));

        std::fs::write(dir.path().join("schema.dbml"), "Table users {\n  id int [pk]\n}\nTable posts {\n  id int [pk]\n  user_id int [ref: > users.id]\n}\n").unwrap();
        let id = DiagramId::parse("schema.dbml").unwrap();
        let file = load(&repo, &id).unwrap();
        assert_eq!(file.editable, Editable::Annotations);
        assert_eq!(file.diagram.nodes.len(), 2);
        assert_eq!(file.diagram.edges[0].from, "public.posts");
        assert_eq!(file.diagram.nodes[0].details, ["id int PK"]);
        let note = Annotation {
            note: Some("Accounts".into()),
            ..Annotation::default()
        };
        let after = edit(
            &repo,
            &id,
            &[
                Edit::Annotate {
                    target: Target::Node("public.users".into()),
                    annotation: note.clone(),
                },
                Edit::Move {
                    id: "public.users".into(),
                    x: 1.0,
                    y: 2.0,
                },
            ],
            &file.hash,
        )
        .unwrap();
        assert_eq!(after.diagram.nodes[0].annotation, note);
        assert!(edit(
            &repo,
            &id,
            &[Edit::RemoveNode {
                id: "public.users".into()
            }],
            &after.hash
        )
        .is_err());

        std::fs::write(dir.path().join("net.drawio"), "<mxfile><diagram id=\"p\" name=\"Network\"><mxGraphModel><root><mxCell id=\"0\"/><mxCell id=\"1\" parent=\"0\"/><mxCell id=\"a\" value=\"A\" style=\"html=1;\" vertex=\"1\" parent=\"1\"><mxGeometry x=\"0\" y=\"0\" width=\"80\" height=\"40\" as=\"geometry\"/></mxCell></root></mxGraphModel></diagram></mxfile>").unwrap();
        let id = DiagramId::parse("net.drawio#0").unwrap();
        let file = load(&repo, &id).unwrap();
        assert_eq!(file.title, "Network");
        let after = edit(
            &repo,
            &id,
            &[Edit::Move {
                id: "a".into(),
                x: 50.0,
                y: 60.0,
            }],
            &file.hash,
        )
        .unwrap();
        assert_eq!(after.diagram.layout["a"].x, 50.0);
        assert!(
            !dir.path()
                .join(".schematlas/layout/net.drawio.0.json")
                .exists(),
            "draw.io keeps its own positions"
        );
    }

    #[test]
    fn resetting_the_layout_returns_to_automatic() {
        let (dir, repo) = repo();
        let file = create(
            &repo,
            ".schematlas/diagrams/flow.mmd",
            "flowchart LR\n  a --> b\n",
        )
        .unwrap();
        let id = DiagramId::parse(&file.id).unwrap();
        let moved = edit(
            &repo,
            &id,
            &[Edit::Move {
                id: "a".into(),
                x: 10.0,
                y: 20.0,
            }],
            &file.hash,
        )
        .unwrap();
        assert_eq!(moved.diagram.layout["a"].x, 10.0);
        let sidecar = dir
            .path()
            .join(".schematlas/layout/.schematlas/diagrams/flow.mmd.json");
        assert!(sidecar.is_file());
        let reset = reset_layout(&repo, &id).unwrap();
        assert!(reset.diagram.layout.is_empty());
        assert!(!sidecar.exists());
        // The diagram's own file is untouched, and resetting again is harmless.
        assert_eq!(reset.hash, file.hash);
        reset_layout(&repo, &id).unwrap();
    }

    #[test]
    fn new_paths_avoid_existing_files() {
        let (_dir, repo) = repo();
        assert_eq!(
            new_path(&repo, "Checkout flow!", ""),
            ".schematlas/diagrams/checkout-flow.mmd"
        );
        repo.write(
            ".schematlas/diagrams/checkout-flow.mmd",
            "flowchart LR\n",
            None,
        )
        .unwrap();
        assert_eq!(
            new_path(&repo, "Checkout flow", ""),
            ".schematlas/diagrams/checkout-flow-2.mmd"
        );
        assert_eq!(
            new_path(&repo, "Whiteboard", &excalidraw::empty()),
            ".schematlas/diagrams/whiteboard.excalidraw"
        );
    }

    #[test]
    fn excalidraw_scenes_are_drawn_in_their_editor_and_annotated_here() {
        let (_dir, repo) = repo();
        let path = new_path(&repo, "Sketch", &excalidraw::empty());
        let file = create(&repo, &path, &excalidraw::empty()).unwrap();
        assert_eq!(file.format, Format::Excalidraw);
        assert_eq!(file.editable, Editable::Annotations);
        assert_eq!(file.id, path);
        // The editor saves the whole scene, compare-and-swap.
        let scene = r#"{"type":"excalidraw","version":2,"elements":[
            {"id":"api","type":"rectangle","x":0,"y":0,"width":100,"height":60},
            {"id":"t","type":"text","x":10,"y":10,"width":30,"height":20,"text":"API","containerId":"api"},
            {"id":"db","type":"ellipse","x":200,"y":0,"width":100,"height":60},
            {"id":"a","type":"arrow","x":100,"y":30,"width":100,"height":0,
             "startBinding":{"elementId":"api"},"endBinding":{"elementId":"db"},"endArrowhead":"arrow"}
        ],"appState":{},"files":{}}"#;
        let id = DiagramId::parse(&path).unwrap();
        let saved = restore(&repo, &id, scene, &file.hash).unwrap();
        assert_eq!(saved.diagram.nodes.len(), 2);
        assert_eq!(saved.diagram.edges.len(), 1);
        assert_eq!(saved.diagram.layout["db"].x, 200.0);
        assert!(
            restore(&repo, &id, scene, &file.hash).is_err(),
            "stale base"
        );
        assert!(restore(&repo, &id, "{\"type\":\"other\"}", &saved.hash).is_err());
        let listed = entries(&path, scene, &saved.hash);
        assert_eq!(
            (listed[0].kind.as_str(), listed[0].nodes),
            ("excalidraw", 2)
        );
        // Annotations are written into the scene; structure and positions are not.
        let note = model::Annotation {
            note: Some("Serves checkout".into()),
            code: vec![model::CodeRef::parse("src/api.rs:3-9")],
            ..model::Annotation::default()
        };
        let annotated = edit(
            &repo,
            &id,
            &[Edit::Annotate {
                target: Target::Node("api".into()),
                annotation: note.clone(),
            }],
            &saved.hash,
        )
        .unwrap();
        assert_eq!(annotated.diagram.nodes[0].annotation, note);
        assert!(annotated.text.contains("\"customData\""));
        assert_eq!(annotated.diagram.layout["db"].x, 200.0);
        for refused in [
            Edit::AddNode {
                id: "cache".into(),
                label: "Cache".into(),
                shape: None,
            },
            Edit::Move {
                id: "api".into(),
                x: 5.0,
                y: 5.0,
            },
        ] {
            assert!(edit(&repo, &id, &[refused], &annotated.hash).is_err());
        }
        assert!(reset_layout(&repo, &id).is_err());
    }

    #[test]
    fn sketches_of_a_flowchart_follow_it_both_ways() {
        let (_dir, repo) = repo();
        let flow = create(
            &repo,
            ".schematlas/diagrams/checkout.mmd",
            "flowchart LR\n  api[API] --> db[(DB)]\n",
        )
        .unwrap();
        let source = DiagramId::parse(&flow.id).unwrap();
        let layout = BTreeMap::from([(
            "api".to_owned(),
            Rect {
                x: 0.0,
                y: 0.0,
                width: 160.0,
                height: 80.0,
            },
        )]);
        let sketch = create_sketch(&repo, &source, &layout).unwrap();
        assert_eq!(
            sketch.path,
            ".schematlas/diagrams/checkout-sketch.excalidraw"
        );
        assert_eq!(sketch.diagram.nodes.len(), 2);
        assert_eq!(sketch.diagram.edges.len(), 1);
        let id = DiagramId::parse(&sketch.id).unwrap();

        // Drawn in the editor: db renamed, a cache shape and an arrow to it.
        let mut scene: serde_json::Value = serde_json::from_str(&sketch.text).unwrap();
        let elements = scene["elements"].as_array_mut().unwrap();
        for e in elements.iter_mut() {
            if e["containerId"] == "db" {
                e["text"] = "Postgres".into();
            }
        }
        elements.push(serde_json::json!({"id":"cache","type":"rectangle","x":0,"y":300,"width":160,"height":80}));
        elements.push(serde_json::json!({"id":"cache-t","type":"text","x":0,"y":300,"width":50,"height":25,"text":"Cache","containerId":"cache"}));
        elements.push(
            serde_json::json!({"id":"a2","type":"arrow","x":0,"y":0,"width":1,"height":1,
            "startBinding":{"elementId":"api"},"endBinding":{"elementId":"cache"}}),
        );
        let drawn = serde_json::to_string_pretty(&scene).unwrap();
        let saved = restore(&repo, &id, &drawn, &sketch.hash).unwrap();
        assert_eq!(saved.text, drawn, "the drawing is kept as drawn");
        let flow = load(&repo, &source).unwrap();
        assert!(
            flow.text.contains("Postgres") && flow.text.contains("cache"),
            "{}",
            flow.text
        );
        assert_eq!(flow.diagram.edges.len(), 2);

        // Changed in the Mermaid file (an agent, git): the sketch follows on the next sync.
        let text = flow.text.replace("Postgres", "Orders DB") + "  api --> queue[Queue]\n";
        restore(&repo, &source, &text, &flow.hash).unwrap();
        let synced = sync_sketch(&repo, &id).unwrap();
        let labels: Vec<_> = synced
            .diagram
            .nodes
            .iter()
            .map(|n| n.label.as_str())
            .collect();
        assert!(
            labels.contains(&"Orders DB") && labels.contains(&"Queue"),
            "{labels:?}"
        );
        assert_eq!(synced.diagram.edges.len(), 3);
        assert_eq!(
            sync_sketch(&repo, &id).unwrap().hash,
            synced.hash,
            "settled"
        );

        // Notes on a sketch's component are the source's.
        let note = model::Annotation {
            note: Some("Entry point".into()),
            ..model::Annotation::default()
        };
        let annotated = edit(
            &repo,
            &id,
            &[Edit::Annotate {
                target: Target::Node("api".into()),
                annotation: note.clone(),
            }],
            &synced.hash,
        )
        .unwrap();
        assert_eq!(annotated.hash, synced.hash, "the sketch file is untouched");
        assert_eq!(
            annotated
                .diagram
                .nodes
                .iter()
                .find(|n| n.id == "api")
                .unwrap()
                .annotation,
            note
        );
        assert!(load(&repo, &source).unwrap().text.contains("Entry point"));
    }
}
