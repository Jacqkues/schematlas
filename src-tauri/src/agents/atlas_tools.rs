//! Agent tools for the repository's diagrams and docs. Reads are free; every write is validated
//! first, shown to the user as a reviewable diff, and written only after approval and only if
//! the file has not changed in the meantime.
use super::{mcp, session::Session, AgentHub};
use crate::atlas::{DocEntry, Listing};
use crate::documents::{self, DiagramEntry, DiagramFile, DiagramId};
use crate::error::{AppError, Result};
use crate::repo::Format;
use schematlas_parser::annotate::Target;
use schematlas_parser::model::{Annotation, CodeRef, Diagram, EdgeRef, Edit};
use serde_json::{json, Value};

pub fn render_listing(listing: &Listing) -> String {
    let Some(root) = &listing.root else {
        return "No repository is open. The user picks one in Schematlas.".into();
    };
    let mut text = format!(
        "repository: {root}{}\n",
        if listing.is_git {
            ""
        } else {
            " (not a git repository)"
        }
    );
    if listing.diagrams.is_empty() && listing.docs.is_empty() {
        text.push_str(
            "No diagrams or docs yet. Create one with write_diagram (Mermaid) or write_doc.",
        );
        return text;
    }
    for e in &listing.diagrams {
        text.push_str(&format!("diagram {} [{}] \"{}\"", e.id, e.kind, e.title));
        match &e.error {
            Some(error) => text.push_str(&format!(" INVALID: {error}")),
            None => text.push_str(&format!(" {} nodes, {} edges", e.nodes, e.edges)),
        }
        match e.editable {
            documents::Editable::Annotations => text.push_str(" (annotations only)"),
            documents::Editable::None => text.push_str(" (read-only)"),
            documents::Editable::Full => {}
        }
        text.push('\n');
    }
    for d in &listing.docs {
        text.push_str(&format!("doc {} \"{}\"\n", d.slug, d.title));
    }
    text.truncate(text.trim_end().len());
    text
}

/// Match an id, a path or a (case-insensitive) title.
pub fn find<'a>(entries: &'a [DiagramEntry], reference: &str) -> Option<&'a DiagramEntry> {
    let reference = reference.trim();
    entries
        .iter()
        .find(|e| e.id == reference)
        .or_else(|| entries.iter().find(|e| e.path == reference))
        .or_else(|| {
            entries
                .iter()
                .find(|e| e.title.eq_ignore_ascii_case(reference))
        })
}
/// Docs by slug (nested TOC docs by their path inside `.schematlas/`), file path or title.
fn find_doc<'a>(docs: &'a [DocEntry], reference: &str) -> Option<&'a DocEntry> {
    let reference = reference.trim();
    let flat = reference.trim_end_matches(".md");
    let bare = flat.rsplit('/').next().unwrap_or(flat);
    docs.iter()
        .find(|d| d.slug == reference || d.path == reference)
        .or_else(|| docs.iter().find(|d| d.slug == bare))
        .or_else(|| {
            docs.iter()
                .find(|d| d.title.eq_ignore_ascii_case(reference))
        })
}
fn missing(reference: &str, names: Vec<&str>) -> AppError {
    let names: Vec<&str> = names.into_iter().take(40).collect();
    AppError::Validation(format!(
        "Nothing named `{reference}`. Existing: {}",
        if names.is_empty() {
            "none".into()
        } else {
            names.join(", ")
        }
    ))
}

fn annotation_text(a: &Annotation) -> String {
    let mut out = String::new();
    if let Some(kind) = &a.kind {
        out.push_str(&format!(" kind={kind}"));
    }
    if !a.code.is_empty() {
        let code: Vec<String> = a.code.iter().map(|c| c.to_string()).collect();
        out.push_str(&format!(" code={}", code.join(",")));
    }
    if !a.tags.is_empty() {
        out.push_str(&format!(" #{}", a.tags.join(" #")));
    }
    if let Some(note) = &a.note {
        out.push_str(&format!(" note: {}", note.replace('\n', " ")));
    }
    out
}

/// Compact text for the agent: one line per node, edge and group, ids first.
pub fn render(file: &DiagramFile) -> String {
    let d = &file.diagram;
    let mut text = format!(
        "diagram {} [{}] \"{}\" — {} nodes, {} edges\n",
        file.id,
        d.kind,
        file.title,
        d.nodes.len(),
        d.edges.len()
    );
    if !d.annotation.is_empty() {
        text.push_str(&format!("about:{}\n", annotation_text(&d.annotation)));
    }
    for n in &d.nodes {
        text.push_str(&format!("node {} \"{}\"", n.id, n.label));
        if let Some(shape) = &n.shape {
            text.push_str(&format!(" ({shape})"));
        }
        text.push_str(&annotation_text(&n.annotation));
        text.push('\n');
        for line in &n.details {
            text.push_str(&format!("  {line}\n"));
        }
    }
    for e in &d.edges {
        text.push_str(&format!("edge {} -> {}", e.from, e.to));
        if let Some(label) = &e.label {
            text.push_str(&format!(" \"{label}\""));
        }
        if let (Some(a), Some(b)) = (&e.tail_label, &e.head_label) {
            text.push_str(&format!(" [{a}..{b}]"));
        }
        text.push_str(&annotation_text(&e.annotation));
        text.push('\n');
    }
    for g in &d.groups {
        text.push_str(&format!(
            "group {} \"{}\": {}\n",
            g.id,
            g.title,
            g.nodes.join(", ")
        ));
    }
    text.truncate(text.trim_end().len());
    text
}

fn node_id(d: &Diagram, reference: &str) -> Result<String> {
    let reference = reference.trim();
    if d.nodes.iter().any(|n| n.id == reference) {
        return Ok(reference.to_owned());
    }
    let matches: Vec<&str> = d
        .nodes
        .iter()
        .filter(|n| n.label.eq_ignore_ascii_case(reference))
        .map(|n| n.id.as_str())
        .collect();
    match matches.as_slice() {
        [one] => Ok((*one).to_owned()),
        [] => Err(missing(
            reference,
            d.nodes.iter().map(|n| n.id.as_str()).collect(),
        )),
        _ => Err(AppError::Validation(format!(
            "Several nodes are labelled `{reference}`; use one of their ids: {}",
            matches.join(", ")
        ))),
    }
}
fn end_id(d: &Diagram, reference: &str) -> Result<String> {
    node_id(d, reference).or_else(|error| {
        d.groups
            .iter()
            .find(|g| g.id == reference || g.title.eq_ignore_ascii_case(reference))
            .map(|g| g.id.clone())
            .ok_or(error)
    })
}

/// Agent operations as parser edits, resolving labels to ids against the diagram as it evolves.
fn edits(d: &Diagram, ops: Vec<mcp::DiagramOp>) -> Result<Vec<Edit>> {
    use mcp::DiagramOp as Op;
    let mut d = d.clone();
    let mut out = vec![];
    for op in ops {
        let edit = match op {
            Op::AddNode { id, label, shape } => Edit::AddNode { id, label, shape },
            Op::SetLabel { node, label } => Edit::SetLabel {
                id: node_id(&d, &node)?,
                label,
            },
            Op::RemoveNode { node } => Edit::RemoveNode {
                id: node_id(&d, &node)?,
            },
            Op::AddEdge { from, to, label } => Edit::AddEdge {
                from: end_id(&d, &from)?,
                to: end_id(&d, &to)?,
                label,
            },
            Op::RemoveEdge { from, to, nth } => Edit::RemoveEdge(EdgeRef {
                from: end_id(&d, &from)?,
                to: end_id(&d, &to)?,
                nth: nth.unwrap_or(0),
            }),
            Op::SetEdgeLabel {
                from,
                to,
                nth,
                label,
            } => Edit::SetEdgeLabel {
                edge: EdgeRef {
                    from: end_id(&d, &from)?,
                    to: end_id(&d, &to)?,
                    nth: nth.unwrap_or(0),
                },
                label,
            },
            Op::Annotate {
                node,
                code,
                note,
                kind,
                tags,
            } => {
                let code: Vec<CodeRef> = code.iter().map(|c| CodeRef::parse(c)).collect();
                for c in &code {
                    c.validate()
                        .map_err(|e| AppError::Validation(e.to_string()))?;
                }
                let annotation = Annotation {
                    code,
                    note,
                    kind,
                    tags,
                    ..Annotation::default()
                };
                let target = match node {
                    Some(node) => Target::Node(node_id(&d, &node)?),
                    None => Target::Diagram,
                };
                Edit::Annotate { target, annotation }
            }
        };
        d = schematlas_parser::model::apply(&d, &edit)
            .map_err(|e| AppError::Validation(e.to_string()))?;
        out.push(edit);
    }
    Ok(out)
}

/// A unified diff of two texts, capped for the review card.
pub fn unified_diff(old: &str, new: &str, path: &str) -> String {
    let diff = similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .context_radius(3)
        .header(path, path)
        .to_string();
    super::types::bounded(&diff, 64 * 1024)
}

/// Added, changed and removed nodes and edges, by label, for the review card.
fn change(before: Option<&Diagram>, after: &Diagram) -> Value {
    use schematlas_parser::model::Edge;
    let empty = Diagram::default();
    let before = before.unwrap_or(&empty);
    let label = |d: &Diagram, id: &str| {
        d.nodes
            .iter()
            .find(|n| n.id == id)
            .map_or(id.to_owned(), |n| n.label.clone())
    };
    let edge = |d: &Diagram, e: &Edge| {
        let mut text = format!("{} → {}", label(d, &e.from), label(d, &e.to));
        if let Some(l) = &e.label {
            text.push_str(&format!(" ({l})"));
        }
        text
    };
    let has = |list: &[schematlas_parser::model::Node], id: &str| list.iter().any(|n| n.id == id);
    let added: Vec<String> = after
        .nodes
        .iter()
        .filter(|n| !has(&before.nodes, &n.id))
        .map(|n| n.label.clone())
        .collect();
    let changed: Vec<String> = after
        .nodes
        .iter()
        .filter(|n| before.nodes.iter().any(|b| b.id == n.id && b != *n))
        .map(|n| n.label.clone())
        .collect();
    let removed: Vec<String> = before
        .nodes
        .iter()
        .filter(|n| !has(&after.nodes, &n.id))
        .map(|n| n.label.clone())
        .collect();
    let key = |e: &Edge| (e.from.clone(), e.to.clone(), e.label.clone());
    let added_edges: Vec<String> = after
        .edges
        .iter()
        .filter(|e| !before.edges.iter().any(|b| key(b) == key(e)))
        .map(|e| edge(after, e))
        .collect();
    let removed_edges: Vec<String> = before
        .edges
        .iter()
        .filter(|e| !after.edges.iter().any(|a| key(a) == key(e)))
        .map(|e| edge(before, e))
        .collect();
    json!({
        "summary": format!(
            "+{} ~{} −{} nodes · +{} −{} edges",
            added.len(), changed.len(), removed.len(), added_edges.len(), removed_edges.len()
        ),
        "change": {
            "addedNodes": added,
            "changedNodes": changed,
            "removedNodes": removed,
            "addedEdges": added_edges,
            "removedEdges": removed_edges,
        }
    })
}

fn stale(error: AppError) -> AppError {
    match error {
        AppError::Validation(message) if message.contains("changed since") => AppError::Validation(
            "The file changed while the proposal was under review; read it again before proposing."
                .into(),
        ),
        other => other,
    }
}

fn is_markdown(name: &str) -> bool {
    let name = name.trim();
    name.ends_with(".md") || name.ends_with(".markdown")
}

fn slugify(text: &str) -> String {
    let mut slug: String = text
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug: String = slug.trim_matches('-').chars().take(64).collect();
    if slug.is_empty() {
        "doc".into()
    } else {
        slug
    }
}

impl AgentHub {
    async fn diagram_entry(&self, project_id: &str, reference: &str) -> Result<DiagramEntry> {
        let listing = self.workspace.list_repo(project_id).await?;
        find(&listing.diagrams, reference).cloned().ok_or_else(|| {
            missing(
                reference,
                listing.diagrams.iter().map(|e| e.id.as_str()).collect(),
            )
        })
    }

    pub(super) async fn atlas_tool(
        &self,
        session: &Session,
        project_id: &str,
        name: &str,
        args: Value,
    ) -> Result<Value> {
        let workspace = &self.workspace;
        let text = match name {
            "list_diagrams" => render_listing(&workspace.list_repo(project_id).await?),
            "repo_outline" => {
                let repo = workspace.repo(project_id).await?;
                tokio::task::spawn_blocking(move || crate::wiki::outline(&repo))
                    .await
                    .map_err(|_| {
                        AppError::Validation("Reading the repository was interrupted.".into())
                    })?
            }
            "get_diagram" => {
                let args: mcp::DiagramRef = serde_json::from_value(args)?;
                let e = self.diagram_entry(project_id, &args.diagram).await?;
                if let Some(error) = &e.error {
                    return Err(AppError::Validation(format!(
                        "{} does not parse: {error}",
                        e.id
                    )));
                }
                render(&workspace.get_diagram(project_id, &e.id).await?)
            }
            "read_diagram_source" => {
                let args: mcp::DiagramRef = serde_json::from_value(args)?;
                let e = self.diagram_entry(project_id, &args.diagram).await?;
                if e.format == Format::Drawio {
                    return Err(AppError::Validation(
                        "draw.io XML is not shown; use get_diagram and edit_diagram.".into(),
                    ));
                }
                workspace.get_diagram(project_id, &e.id).await?.source
            }
            "write_diagram" => {
                let args: mcp::WriteDiagramArgs = serde_json::from_value(args)?;
                if Format::of(&args.path) != Some(Format::Mermaid) {
                    return Err(AppError::Validation(
                        "write_diagram writes .mmd files; use edit_diagram for other formats."
                            .into(),
                    ));
                }
                crate::repo::valid_relative(&args.path)?;
                let proposed = schematlas_parser::mermaid::parse(&args.mermaid)
                    .map_err(|e| AppError::Validation(format!("The Mermaid does not parse: {e}")))?
                    .diagram;
                let id = DiagramId::parse(&args.path)?;
                let existing = workspace.get_diagram(project_id, &id.text()).await.ok();
                let old = existing.as_ref().map_or("", |f| f.text.as_str());
                let mut details = change(existing.as_ref().map(|f| &f.diagram), &proposed);
                details["path"] = json!(args.path);
                details["isNew"] = json!(existing.is_none());
                details["diff"] = json!(unified_diff(old, &args.mermaid, &args.path));
                self.approve(
                    session,
                    format!("Diagram · {}", args.path),
                    "diagram",
                    details,
                )
                .await?;
                match existing {
                    Some(file) => {
                        workspace
                            .restore_diagram(project_id, &file.id, args.mermaid, file.hash)
                            .await
                            .map_err(stale)?;
                    }
                    None => {
                        workspace
                            .create_diagram(project_id, Some(args.path.clone()), "", args.mermaid)
                            .await?;
                    }
                }
                format!("Saved {}. The user sees it in Schematlas.", args.path)
            }
            "edit_diagram" => {
                let args: mcp::EditDiagramArgs = serde_json::from_value(args)?;
                let e = self.diagram_entry(project_id, &args.diagram).await?;
                let current = workspace.get_diagram(project_id, &e.id).await?;
                let edits = edits(&current.diagram, args.ops)?;
                let repo = workspace.repo(project_id).await?;
                let id = DiagramId::parse(&current.id)?;
                let planned = documents::plan(&repo, &id, &current, &edits)?;
                let mut details = change(Some(&current.diagram), &planned.diagram);
                let summary = details["summary"].as_str().unwrap_or("").to_owned();
                details["path"] = json!(current.id);
                details["isNew"] = json!(false);
                details["diff"] = json!(unified_diff(&current.text, &planned.text, &current.path));
                self.approve(
                    session,
                    format!("Diagram · {}", current.title),
                    "diagram",
                    details,
                )
                .await?;
                workspace
                    .edit_diagram(project_id, &current.id, edits, current.hash)
                    .await
                    .map_err(stale)?;
                format!("Saved {} ({summary}).", current.path)
            }
            "show_in_app" => {
                let args: mcp::ShowArgs = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let project = workspace.repository.get(project_id).await?;
                let (kind, id, node, title) = if let Some(e) = find(&listing.diagrams, &args.item) {
                    let node = match &args.node {
                        Some(reference) => {
                            let file = workspace.get_diagram(project_id, &e.id).await?;
                            Some(node_id(&file.diagram, reference)?)
                        }
                        None => None,
                    };
                    ("diagram", e.id.clone(), node, e.title.clone())
                } else if let Some(d) = find_doc(&listing.docs, &args.item) {
                    ("doc", d.slug.clone(), None, d.title.clone())
                } else if let Ok(source) = super::schema_tools::resolve_source(&project, &args.item)
                {
                    ("source", source.id.clone(), None, source.name.clone())
                } else {
                    let mut known: Vec<&str> =
                        listing.diagrams.iter().map(|e| e.id.as_str()).collect();
                    known.extend(listing.docs.iter().map(|d| d.slug.as_str()));
                    known.extend(project.sources.iter().map(|s| s.name.as_str()));
                    return Err(missing(&args.item, known));
                };
                if args.node.is_some() && kind != "diagram" {
                    return Err(AppError::Validation(
                        "Only diagrams have nodes to select.".into(),
                    ));
                }
                let shown = match &node {
                    Some(node) => format!("Showing {title}, node `{node}` selected."),
                    None => format!("Showing {title}."),
                };
                let _ = self.navigation.send(super::types::Navigation {
                    project_id: project_id.into(),
                    kind: kind.into(),
                    id,
                    node,
                });
                shown
            }
            "move_diagram_nodes" => {
                let args: mcp::MoveDiagramNodesArgs = serde_json::from_value(args)?;
                let e = self.diagram_entry(project_id, &args.diagram).await?;
                let current = workspace.get_diagram(project_id, &e.id).await?;
                let mut edits = Vec::with_capacity(args.nodes.len());
                for m in &args.nodes {
                    if !m.x.is_finite() || !m.y.is_finite() {
                        return Err(AppError::Validation("Positions must be finite.".into()));
                    }
                    edits.push(Edit::Move {
                        id: node_id(&current.diagram, &m.node)?,
                        x: m.x,
                        y: m.y,
                    });
                }
                let moved = edits.len();
                // draw.io keeps positions in the diagram file itself, so that is a reviewed
                // write; other formats keep an arrangement beside the file, like a drag.
                if e.format == Format::Drawio {
                    let repo = workspace.repo(project_id).await?;
                    let id = DiagramId::parse(&current.id)?;
                    let planned = documents::plan(&repo, &id, &current, &edits)?;
                    let mut details = change(Some(&current.diagram), &planned.diagram);
                    details["path"] = json!(current.id);
                    details["isNew"] = json!(false);
                    details["diff"] =
                        json!(unified_diff(&current.text, &planned.text, &current.path));
                    self.approve(
                        session,
                        format!("Arrange · {}", current.title),
                        "diagram",
                        details,
                    )
                    .await?;
                }
                workspace
                    .edit_diagram(project_id, &current.id, edits, current.hash)
                    .await
                    .map_err(stale)?;
                format!(
                    "Moved {moved} node{} in {}.",
                    if moved == 1 { "" } else { "s" },
                    current.path
                )
            }
            "read_doc" => {
                let args: mcp::DocRef = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let d = find_doc(&listing.docs, &args.doc).ok_or_else(|| {
                    missing(
                        &args.doc,
                        listing.docs.iter().map(|d| d.slug.as_str()).collect(),
                    )
                })?;
                workspace.get_doc(project_id, &d.slug).await?.markdown
            }
            "write_doc" => {
                let args: mcp::WriteDocArgs = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let (slug, base, old) = match find_doc(&listing.docs, &args.doc) {
                    Some(d) => {
                        let doc = workspace.get_doc(project_id, &d.slug).await?;
                        (doc.slug, Some(doc.hash), doc.markdown)
                    }
                    // A new page named by its path (`.schematlas/wiki/overview.md`) goes there;
                    // a plain name becomes `.schematlas/docs/<slug>.md`.
                    None if is_markdown(&args.doc) => {
                        let path = args.doc.trim().trim_start_matches("./");
                        let path = if path.starts_with(".schematlas/") {
                            path.to_owned()
                        } else {
                            format!("{}/{path}", crate::repo::ATLAS_DIR)
                        };
                        (path, None, String::new())
                    }
                    None => (slugify(&args.doc), None, String::new()),
                };
                let path = if is_markdown(&slug) {
                    slug.clone()
                } else {
                    format!("{}/docs/{slug}.md", crate::repo::ATLAS_DIR)
                };
                // Citations and diagram links must resolve before anyone is asked to review.
                let repo = workspace.repo(project_id).await?;
                let diagrams: std::collections::HashSet<String> =
                    listing.diagrams.iter().map(|d| d.id.clone()).collect();
                let markdown = args.markdown.clone();
                let problems = tokio::task::spawn_blocking(move || {
                    crate::wiki::check_page(&repo, &diagrams, &markdown)
                })
                .await
                .map_err(|_| AppError::Validation("Checking the page was interrupted.".into()))?;
                if !problems.is_empty() {
                    return Err(AppError::Validation(format!(
                        "{path} was not proposed; fix these and write it again:\n- {}",
                        problems.join("\n- ")
                    )));
                }
                let title = crate::repo::doc_title(&args.markdown).unwrap_or_else(|| slug.clone());
                let details = json!({
                    "path": path,
                    "isNew": base.is_none(),
                    "title": title,
                    "diff": unified_diff(&old, &args.markdown, &path),
                });
                self.approve(session, format!("Doc · {title}"), "doc", details)
                    .await?;
                workspace
                    .save_doc(project_id, &slug, args.markdown, base)
                    .await
                    .map_err(stale)?;
                format!("Saved {path}.")
            }
            "check_refs" => {
                let args: mcp::CheckRefsArgs = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let chosen: Vec<DiagramEntry> = match &args.diagram {
                    Some(reference) => vec![self.diagram_entry(project_id, reference).await?],
                    None => listing
                        .diagrams
                        .iter()
                        .filter(|e| e.error.is_none())
                        .cloned()
                        .collect(),
                };
                let repo = workspace.repo(project_id).await?;
                let mut broken = vec![];
                let mut checked = 0;
                for e in chosen {
                    let file = workspace.get_diagram(project_id, &e.id).await?;
                    let d = &file.diagram;
                    let refs = d
                        .nodes
                        .iter()
                        .flat_map(|n| n.annotation.code.iter().map(move |r| (n.id.clone(), r)))
                        .chain(d.edges.iter().flat_map(|x| {
                            x.annotation
                                .code
                                .iter()
                                .map(move |r| (format!("{}->{}", x.from, x.to), r))
                        }));
                    for (owner, code) in refs {
                        checked += 1;
                        if let Err(error) = repo.snippet(code, 0) {
                            broken.push(format!("{} {owner}: {error}", file.id));
                        }
                    }
                }
                if broken.is_empty() {
                    format!("All {checked} code references resolve.")
                } else {
                    format!(
                        "{} of {checked} code references are broken:\n{}",
                        broken.len(),
                        broken.join("\n")
                    )
                }
            }
            _ => return Err(AppError::Validation("Unknown project tool.".into())),
        };
        Ok(Value::String(text))
    }
}
