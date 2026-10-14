//! Agent tools for the repository atlas. Reads are free; every write is validated first, shown to
//! the user as a reviewable change, and written only after approval and only if the file has not
//! changed in the meantime.
use super::{mcp, session::Session, AgentHub};
use crate::{
    diagram::{self, Change, CodeRef, Diagram, Edge, Group, Node},
    error::{AppError, Result},
    repo::{Entry, Kind, Listing},
};
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
        text.push_str("No diagrams or docs yet. Create them with propose_diagram and write_doc.");
        return text;
    }
    for entry in &listing.diagrams {
        text.push_str(&format!("diagram {} \"{}\"", entry.slug, entry.title));
        match &entry.error {
            Some(error) => text.push_str(&format!(" INVALID: {error}")),
            None => text.push_str(&format!(
                " [{}] {} nodes, {} edges",
                entry.kind.as_deref().unwrap_or("?"),
                entry.nodes,
                entry.edges
            )),
        }
        text.push('\n');
    }
    for entry in &listing.docs {
        text.push_str(&format!("doc {} \"{}\"", entry.slug, entry.title));
        if let Some(error) = &entry.error {
            text.push_str(&format!(" INVALID: {error}"));
        }
        text.push('\n');
    }
    text.truncate(text.trim_end().len());
    text
}

/// Match a slug, file name or (case-insensitive) title.
pub fn find<'a>(entries: &'a [Entry], reference: &str) -> Option<&'a Entry> {
    let reference = reference.trim();
    let bare = reference
        .rsplit('/')
        .next()
        .unwrap_or(reference)
        .trim_end_matches(".json")
        .trim_end_matches(".md");
    entries
        .iter()
        .find(|e| e.slug == reference || e.path == reference)
        .or_else(|| entries.iter().find(|e| e.slug == bare))
        .or_else(|| {
            entries
                .iter()
                .find(|e| e.title.eq_ignore_ascii_case(reference))
        })
}
fn missing(what: &str, reference: &str, entries: &[Entry]) -> AppError {
    let names: Vec<&str> = entries.iter().take(40).map(|e| e.slug.as_str()).collect();
    AppError::Validation(format!(
        "No {what} `{reference}`. Existing: {}",
        if names.is_empty() {
            "none".into()
        } else {
            names.join(", ")
        }
    ))
}

/// Diagram JSON (object or text) or a Mermaid flowchart.
pub fn parse_content(title: &str, content: &Value) -> Result<Diagram> {
    let mut diagram: Diagram = match content {
        Value::String(text) => {
            let trimmed = text.trim_start();
            if trimmed.starts_with('{') {
                serde_json::from_str(trimmed)?
            } else {
                return crate::mermaid::parse(title, text);
            }
        }
        Value::Object(_) => serde_json::from_value(content.clone())?,
        _ => {
            return Err(AppError::Validation(
                "content is a diagram object or a Mermaid flowchart string.".into(),
            ))
        }
    };
    if diagram.title.trim().is_empty() {
        diagram.title = title.to_owned();
    }
    diagram.normalize();
    diagram.validate()?;
    Ok(diagram)
}

fn refs(values: Option<Vec<String>>) -> Option<Vec<CodeRef>> {
    values.map(|v| v.iter().map(|r| CodeRef::parse(r)).collect())
}

/// Apply edit operations to a copy; any failure leaves the diagram untouched.
pub fn apply(diagram: &Diagram, ops: Vec<mcp::DiagramOp>) -> Result<Diagram> {
    use mcp::DiagramOp as Op;
    let mut d = diagram.clone();
    for op in ops {
        match op {
            Op::AddNode {
                id,
                label,
                kind,
                body_md,
                tags,
                code_refs,
            } => {
                let id = id.unwrap_or_else(|| {
                    diagram::unused_slug(&label, |s| d.nodes.iter().any(|n| n.id == s))
                });
                if d.nodes.iter().any(|n| n.id == id) {
                    return Err(AppError::Validation(format!("Node `{id}` already exists.")));
                }
                d.nodes.push(Node {
                    id,
                    kind: kind.unwrap_or_else(|| "module".into()),
                    label,
                    body_md,
                    tags: tags.unwrap_or_default(),
                    code_refs: refs(code_refs).unwrap_or_default(),
                    ..Node::default()
                });
            }
            Op::UpdateNode {
                node,
                label,
                kind,
                body_md,
                tags,
                code_refs,
            } => {
                let id = d.resolve_node(&node)?.id.clone();
                let target = d.nodes.iter_mut().find(|n| n.id == id).expect("resolved");
                if let Some(label) = label {
                    target.label = label;
                }
                if let Some(kind) = kind {
                    target.kind = kind;
                }
                if let Some(body) = body_md {
                    target.body_md = Some(body).filter(|b| !b.trim().is_empty());
                }
                if let Some(tags) = tags {
                    target.tags = tags;
                }
                if let Some(code_refs) = refs(code_refs) {
                    target.code_refs = code_refs;
                }
            }
            Op::RemoveNode { node } => {
                let id = d.resolve_node(&node)?.id.clone();
                d.nodes.retain(|n| n.id != id);
                d.edges.retain(|e| e.source != id && e.target != id);
                for group in &mut d.groups {
                    group.node_ids.retain(|n| *n != id);
                }
                d.groups.retain(|g| !g.node_ids.is_empty());
                d.layout.remove(&id);
            }
            Op::AddEdge {
                from,
                to,
                label,
                kind,
                code_refs,
            } => {
                let source = d.resolve_node(&from)?.id.clone();
                let target = d.resolve_node(&to)?.id.clone();
                d.edges.push(Edge {
                    source,
                    target,
                    label,
                    kind,
                    code_refs: refs(code_refs).unwrap_or_default(),
                    ..Edge::default()
                });
                d.normalize();
            }
            Op::RemoveEdge { from, to } => {
                let source = d.resolve_node(&from)?.id.clone();
                let target = d.resolve_node(&to)?.id.clone();
                let before = d.edges.len();
                d.edges
                    .retain(|e| !(e.source == source && e.target == target));
                if d.edges.len() == before {
                    return Err(AppError::Validation(format!(
                        "No edge from `{from}` to `{to}`."
                    )));
                }
            }
            Op::SetGroup { name, nodes, color } => {
                let mut ids = vec![];
                for node in &nodes {
                    ids.push(d.resolve_node(node)?.id.clone());
                }
                match d
                    .groups
                    .iter_mut()
                    .find(|g| g.name.eq_ignore_ascii_case(&name))
                {
                    Some(group) => {
                        group.node_ids = ids;
                        if let Some(color) = color {
                            group.color = color;
                        }
                    }
                    None => {
                        let id =
                            diagram::unused_slug(&name, |s| d.groups.iter().any(|g| g.id == s));
                        d.groups.push(Group {
                            id,
                            name,
                            node_ids: ids,
                            color: color.unwrap_or_else(crate::domain::default_group_color),
                        });
                    }
                }
                d.groups.retain(|g| !g.node_ids.is_empty());
            }
            Op::RemoveGroup { name } => {
                let before = d.groups.len();
                d.groups
                    .retain(|g| !(g.name.eq_ignore_ascii_case(&name) || g.id == name));
                if d.groups.len() == before {
                    return Err(AppError::Validation(format!("No group `{name}`.")));
                }
            }
        }
    }
    d.normalize();
    d.validate()?;
    Ok(d)
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

impl AgentHub {
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
            "get_diagram" => {
                let args: mcp::DiagramRef = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let entry = find(&listing.diagrams, &args.diagram)
                    .ok_or_else(|| missing("diagram", &args.diagram, &listing.diagrams))?;
                if let Some(error) = &entry.error {
                    return Err(AppError::Validation(format!(
                        "{} is invalid: {error}. Replace it with propose_diagram.",
                        entry.path
                    )));
                }
                let file = workspace.get_diagram(project_id, &entry.slug).await?;
                diagram::render(&file.slug, &file.diagram)
            }
            "propose_diagram" => {
                let args: mcp::ProposeDiagramArgs = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let existing = find(&listing.diagrams, &args.diagram).cloned();
                let title = args
                    .title
                    .clone()
                    .or_else(|| existing.as_ref().map(|e| e.title.clone()))
                    .unwrap_or_else(|| args.diagram.clone());
                let mut proposed = parse_content(&title, &args.content)?;
                if let Some(title) = args.title {
                    proposed.title = title;
                }
                let (slug, base, previous) = match &existing {
                    Some(entry) => {
                        let previous = workspace
                            .get_diagram(project_id, &entry.slug)
                            .await
                            .ok()
                            .map(|f| f.diagram);
                        (entry.slug.clone(), Some(entry.hash.clone()), previous)
                    }
                    None => {
                        let slug = diagram::slugify(&args.diagram);
                        diagram::valid_slug(&slug)?;
                        (slug, None, None)
                    }
                };
                if let Some(previous) = &previous {
                    proposed.inherit_layout(previous);
                }
                self.review_diagram(
                    session,
                    project_id,
                    &slug,
                    proposed,
                    previous.as_ref(),
                    base,
                )
                .await?
            }
            "update_diagram" => {
                let args: mcp::UpdateDiagramArgs = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let entry = find(&listing.diagrams, &args.diagram)
                    .ok_or_else(|| missing("diagram", &args.diagram, &listing.diagrams))?;
                let file = workspace.get_diagram(project_id, &entry.slug).await?;
                let updated = apply(&file.diagram, args.ops)?;
                self.review_diagram(
                    session,
                    project_id,
                    &file.slug,
                    updated,
                    Some(&file.diagram),
                    Some(file.hash.clone()),
                )
                .await?
            }
            "read_doc" => {
                let args: mcp::DocRef = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let entry = find(&listing.docs, &args.doc)
                    .ok_or_else(|| missing("doc", &args.doc, &listing.docs))?;
                workspace.get_doc(project_id, &entry.slug).await?.markdown
            }
            "write_doc" => {
                let args: mcp::WriteDocArgs = serde_json::from_value(args)?;
                let listing = workspace.list_repo(project_id).await?;
                let (slug, base, old) = match find(&listing.docs, &args.doc) {
                    Some(entry) => {
                        let doc = workspace.get_doc(project_id, &entry.slug).await?;
                        (doc.slug, Some(doc.hash), doc.markdown)
                    }
                    None => {
                        let slug = diagram::slugify(&args.doc);
                        diagram::valid_slug(&slug)?;
                        (slug, None, String::new())
                    }
                };
                let path = if slug.ends_with(".md") || slug.ends_with(".markdown") {
                    format!(".schematlas/{slug}")
                } else {
                    Kind::Doc.relative(&slug)
                };
                let title = crate::repo::doc_title(&args.markdown).unwrap_or_else(|| slug.clone());
                let details = json!({
                    "path": path,
                    "isNew": base.is_none(),
                    "title": title,
                    "diff": unified_diff(&old, &args.markdown, &path),
                    "markdown": super::types::bounded(&args.markdown, 64 * 1024),
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
                let entries: Vec<&Entry> = match &args.diagram {
                    Some(reference) => vec![find(&listing.diagrams, reference)
                        .ok_or_else(|| missing("diagram", reference, &listing.diagrams))?],
                    None => listing
                        .diagrams
                        .iter()
                        .filter(|e| e.error.is_none())
                        .collect(),
                };
                let repo = workspace.repo(project_id).await?;
                let mut broken = vec![];
                let mut checked = 0;
                for entry in entries {
                    let file = workspace.get_diagram(project_id, &entry.slug).await?;
                    let refs =
                        file.diagram
                            .nodes
                            .iter()
                            .flat_map(|n| n.code_refs.iter().map(move |r| (n.id.as_str(), r)))
                            .chain(
                                file.diagram.edges.iter().flat_map(|e| {
                                    e.code_refs.iter().map(move |r| (e.id.as_str(), r))
                                }),
                            );
                    for (owner, code_ref) in refs {
                        checked += 1;
                        if let Err(error) = repo.snippet(code_ref, 0) {
                            broken.push(format!("{} {owner}: {error}", file.slug));
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

    async fn review_diagram(
        &self,
        session: &Session,
        project_id: &str,
        slug: &str,
        proposed: Diagram,
        previous: Option<&Diagram>,
        base: Option<String>,
    ) -> Result<String> {
        let change = Change::between(previous, &proposed);
        let path = Kind::Diagram.relative(slug);
        let details = json!({
            "path": path,
            "slug": slug,
            "isNew": base.is_none(),
            "title": proposed.title,
            "kind": proposed.kind,
            "summary": change.summary(),
            "change": change,
            "diagram": proposed,
        });
        self.approve(
            session,
            format!("Diagram · {}", proposed.title),
            "diagram",
            details,
        )
        .await?;
        self.workspace
            .save_diagram(project_id, slug, proposed, base)
            .await
            .map_err(stale)?;
        Ok(format!(
            "Saved {path} ({}). The user sees it in Schematlas.",
            change.summary()
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations_edit_by_label_and_fail_atomically() {
        let base = parse_content("Map", &json!("flowchart LR\n api[API] --> db[(DB)]")).unwrap();
        let ops: Vec<mcp::DiagramOp> = serde_json::from_value(json!([
            {"op":"add_node","label":"Cache","kind":"store","code_refs":["src/cache.rs:1-9"]},
            {"op":"add_edge","from":"API","to":"Cache","label":"reads"},
            {"op":"update_node","node":"db","body_md":"Primary store"},
            {"op":"set_group","name":"Data","nodes":["DB","cache"]}
        ]))
        .unwrap();
        let updated = apply(&base, ops).unwrap();
        assert_eq!(updated.nodes.len(), 3);
        assert_eq!(updated.groups[0].node_ids, ["db", "cache"]);
        assert_eq!(updated.edges.len(), 2);
        let failing: Vec<mcp::DiagramOp> = serde_json::from_value(json!([
            {"op":"remove_node","node":"api"},
            {"op":"add_edge","from":"api","to":"db"}
        ]))
        .unwrap();
        assert!(apply(&updated, failing).is_err());
        let removed: Vec<mcp::DiagramOp> =
            serde_json::from_value(json!([{"op":"remove_node","node":"API"}])).unwrap();
        let removed = apply(&updated, removed).unwrap();
        assert!(removed.edges.iter().all(|e| e.source != "api"));
    }
}
