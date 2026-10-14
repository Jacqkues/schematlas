//! Repository diagrams: the `.schematlas/diagrams/<slug>.json` format, its invariants, and the
//! plain-text renderings the agent reads. Files are the source of truth, so unknown fields survive a
//! round-trip and layout is kept apart from meaning to keep diffs readable.
use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

pub const VERSION: u32 = 1;
const MAX_NODES: usize = 2000;
const MAX_EDGES: usize = 8000;
const MAX_GROUPS: usize = 200;
const MAX_TEXT: usize = 16 * 1024;
pub const MAX_FILE: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Diagram {
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_md: Option<String>,
    /// Schema diagrams name a project source instead of copying its catalog.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceRef>,
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layout: BTreeMap<String, Frame>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
fn version() -> u32 {
    VERSION
}
fn default_kind() -> String {
    "architecture".into()
}
fn default_node_kind() -> String {
    "module".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SourceRef {
    pub name: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Node {
    pub id: String,
    #[serde(default = "default_node_kind")]
    pub kind: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_md: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_refs: Vec<CodeRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    #[serde(default)]
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_md: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub code_refs: Vec<CodeRef>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub node_ids: Vec<String>,
    #[serde(default = "crate::domain::default_group_color")]
    pub color: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Style {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub h: Option<f64>,
}

/// A repository location. Written as `path:line-end` in files; the object form adds a symbol.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CodeRef {
    pub path: String,
    pub line: Option<u32>,
    pub end_line: Option<u32>,
    pub symbol: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum CodeRefRepr {
    Short(String),
    Full {
        path: String,
        #[serde(default)]
        line: Option<u32>,
        #[serde(default)]
        end_line: Option<u32>,
        #[serde(default)]
        symbol: Option<String>,
    },
}
impl CodeRef {
    pub fn parse(text: &str) -> CodeRef {
        let text = text.trim();
        let text = text.strip_prefix("./").unwrap_or(text);
        // `path#L5-L20` (Markdown links) and `path:5-20` are both accepted.
        let (path, range) = match text.rsplit_once("#L") {
            Some((path, range)) => (path, Some(range.replace('L', ""))),
            None => match text.rsplit_once(':') {
                Some((path, range))
                    if !range.is_empty()
                        && range.chars().all(|c| c.is_ascii_digit() || c == '-') =>
                {
                    (path, Some(range.to_owned()))
                }
                _ => (text, None),
            },
        };
        let (line, end_line) = match range.as_deref().map(|r| r.split_once('-')) {
            Some(Some((a, b))) => (a.parse().ok(), b.parse().ok()),
            Some(None) => (range.as_deref().and_then(|r| r.parse().ok()), None),
            None => (None, None),
        };
        CodeRef {
            path: path.to_owned(),
            line,
            end_line,
            symbol: None,
        }
    }
    pub fn display(&self) -> String {
        let mut text = self.path.clone();
        if let Some(line) = self.line {
            text.push_str(&format!(":{line}"));
            if let Some(end) = self.end_line.filter(|e| *e != line) {
                text.push_str(&format!("-{end}"));
            }
        }
        text
    }
    pub fn validate(&self) -> Result<()> {
        let path = &self.path;
        let bad = path.is_empty()
            || path.len() > 512
            || path.starts_with('/')
            || path.starts_with('\\')
            || path.contains('\\')
            || path.contains('\0')
            || path.chars().nth(1) == Some(':')
            || path.split('/').any(|part| part == ".." || part.is_empty());
        if bad {
            return Err(AppError::Validation(format!(
                "Code reference `{path}` must be a relative path inside the repository."
            )));
        }
        if matches!((self.line, self.end_line), (Some(0), _) | (_, Some(0)))
            || matches!((self.line, self.end_line), (Some(a), Some(b)) if b < a)
            || (self.line.is_none() && self.end_line.is_some())
        {
            return Err(AppError::Validation(format!(
                "Code reference `{}` has an invalid line range.",
                self.display()
            )));
        }
        Ok(())
    }
}
impl Serialize for CodeRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        match &self.symbol {
            None => s.serialize_str(&self.display()),
            Some(symbol) => {
                use serde::ser::SerializeMap;
                let mut map = s.serialize_map(None)?;
                map.serialize_entry("path", &self.path)?;
                if let Some(line) = self.line {
                    map.serialize_entry("line", &line)?;
                }
                if let Some(end) = self.end_line {
                    map.serialize_entry("end_line", &end)?;
                }
                map.serialize_entry("symbol", symbol)?;
                map.end()
            }
        }
    }
}
impl<'de> Deserialize<'de> for CodeRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Ok(match CodeRefRepr::deserialize(d)? {
            CodeRefRepr::Short(text) => CodeRef::parse(&text),
            CodeRefRepr::Full {
                path,
                line,
                end_line,
                symbol,
            } => CodeRef {
                path: CodeRef::parse(&path).path,
                line,
                end_line,
                symbol: symbol.filter(|s| !s.trim().is_empty()),
            },
        })
    }
}

pub fn valid_slug(slug: &str) -> Result<()> {
    let ok = !slug.is_empty()
        && slug.len() <= 64
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !slug.starts_with('-');
    if ok {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "`{slug}` is not a valid name. Use lowercase letters, digits and dashes (max 64)."
        )))
    }
}
/// `Checkout flow (v2)` → `checkout-flow-v2`.
pub fn slugify(text: &str) -> String {
    let mut slug = String::new();
    for c in text.trim().chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
        if slug.len() >= 64 {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "diagram".into()
    } else {
        slug
    }
}
/// A slug for `title` that `taken` does not claim yet: `map`, `map-2`, `map-3`…
pub fn unused_slug(title: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = slugify(title);
    let mut slug = base.clone();
    let mut n = 2;
    while taken(&slug) {
        let suffix = format!("-{n}");
        slug = format!("{}{suffix}", &base[..base.len().min(64 - suffix.len())]);
        n += 1;
    }
    slug
}
fn valid_token(what: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 40
        || !value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(AppError::Validation(format!(
            "{what} `{value}` must be a short lowercase word such as `service` or `data-flow`."
        )));
    }
    Ok(())
}
pub fn valid_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}
fn text_limit(what: &str, text: Option<&str>, max: usize) -> Result<()> {
    if text.is_some_and(|t| t.len() > max) {
        return Err(AppError::Validation(format!(
            "{what} exceeds {} KB.",
            max / 1024
        )));
    }
    Ok(())
}
fn valid_id(what: &str, id: &str) -> Result<()> {
    if id.trim().is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
        return Err(AppError::Validation(format!(
            "{what} ids must be 1 to 128 printable characters."
        )));
    }
    Ok(())
}

impl Diagram {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > MAX_FILE {
            return Err(AppError::Validation(
                "Diagram files are limited to 2 MB.".into(),
            ));
        }
        let mut diagram: Diagram = serde_json::from_str(text)?;
        diagram.normalize();
        diagram.validate()?;
        Ok(diagram)
    }
    pub fn to_file(&self) -> Result<String> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        if text.len() > MAX_FILE {
            return Err(AppError::Validation(
                "Diagram files are limited to 2 MB.".into(),
            ));
        }
        Ok(text)
    }
    /// Fill derivable fields so authors (people and agents) can leave them out.
    pub fn normalize(&mut self) {
        self.title = self.title.trim().to_owned();
        let mut used = HashSet::new();
        for edge in &mut self.edges {
            if edge.id.trim().is_empty() {
                let base = format!("{}->{}", edge.source, edge.target);
                let mut id = base.clone();
                let mut n = 2;
                while used.contains(&id) {
                    id = format!("{base}#{n}");
                    n += 1;
                }
                edge.id = id;
            }
            used.insert(edge.id.clone());
        }
        for node in &mut self.nodes {
            node.label = node.label.trim().to_owned();
            node.tags.retain(|t| !t.trim().is_empty());
            node.tags.dedup();
        }
        let ids: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        self.layout.retain(|id, _| ids.contains(id.as_str()));
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != VERSION {
            return Err(AppError::Validation(format!(
                "Unsupported diagram version {}; this app reads version {VERSION}.",
                self.version
            )));
        }
        if self.title.is_empty() || self.title.chars().count() > 120 {
            return Err(AppError::Validation(
                "Give the diagram a title between 1 and 120 characters.".into(),
            ));
        }
        valid_token("Diagram kind", &self.kind)?;
        text_limit("The description", self.description_md.as_deref(), MAX_TEXT)?;
        if self.kind == "schema" && self.source.is_none() {
            return Err(AppError::Validation(
                "A schema diagram names its source: \"source\": {\"name\": \"…\"}.".into(),
            ));
        }
        if self.nodes.len() > MAX_NODES
            || self.edges.len() > MAX_EDGES
            || self.groups.len() > MAX_GROUPS
        {
            return Err(AppError::Validation(format!(
                "Diagrams are limited to {MAX_NODES} nodes, {MAX_EDGES} edges and {MAX_GROUPS} groups."
            )));
        }
        let mut ids = HashSet::new();
        for node in &self.nodes {
            valid_id("Node", &node.id)?;
            if !ids.insert(node.id.as_str()) {
                return Err(AppError::Validation(format!(
                    "Node id `{}` is used twice.",
                    node.id
                )));
            }
            valid_token("Node kind", &node.kind)?;
            if node.label.is_empty() || node.label.chars().count() > 200 {
                return Err(AppError::Validation(format!(
                    "Node `{}` needs a label of 1 to 200 characters.",
                    node.id
                )));
            }
            text_limit("A node body", node.body_md.as_deref(), MAX_TEXT)?;
            if node.tags.len() > 20 || node.tags.iter().any(|t| t.len() > 40) {
                return Err(AppError::Validation(
                    "Nodes take at most 20 tags of 40 characters.".into(),
                ));
            }
            if node.code_refs.len() > 50 {
                return Err(AppError::Validation(
                    "Nodes take at most 50 code references.".into(),
                ));
            }
            for code_ref in &node.code_refs {
                code_ref.validate()?;
            }
            if let Some(style) = &node.style {
                if style.color.as_deref().is_some_and(|c| !valid_color(c)) {
                    return Err(AppError::Validation(
                        "Use node colors in #RRGGBB format.".into(),
                    ));
                }
                if let Some(shape) = &style.shape {
                    valid_token("Shape", shape)?;
                }
            }
        }
        let mut edge_ids = HashSet::new();
        for edge in &self.edges {
            valid_id("Edge", &edge.id)?;
            if !edge_ids.insert(edge.id.as_str()) {
                return Err(AppError::Validation(format!(
                    "Edge id `{}` is used twice.",
                    edge.id
                )));
            }
            for end in [&edge.source, &edge.target] {
                if !ids.contains(end.as_str()) {
                    return Err(AppError::Validation(format!(
                        "Edge `{}` points to missing node `{end}`.",
                        edge.id
                    )));
                }
            }
            if edge.label.as_ref().is_some_and(|l| l.chars().count() > 200) {
                return Err(AppError::Validation(
                    "Edge labels are limited to 200 characters.".into(),
                ));
            }
            if let Some(kind) = &edge.kind {
                valid_token("Edge kind", kind)?;
            }
            text_limit("An edge body", edge.body_md.as_deref(), MAX_TEXT)?;
            for code_ref in &edge.code_refs {
                code_ref.validate()?;
            }
        }
        let mut group_ids = HashSet::new();
        for group in &self.groups {
            valid_id("Group", &group.id)?;
            if !group_ids.insert(group.id.as_str()) {
                return Err(AppError::Validation(format!(
                    "Group id `{}` is used twice.",
                    group.id
                )));
            }
            if group.name.trim().is_empty() || group.name.chars().count() > 80 {
                return Err(AppError::Validation(
                    "Group names are 1 to 80 characters.".into(),
                ));
            }
            if !valid_color(&group.color) {
                return Err(AppError::Validation(
                    "Use group colors in #RRGGBB format.".into(),
                ));
            }
            if let Some(missing) = group.node_ids.iter().find(|id| !ids.contains(id.as_str())) {
                return Err(AppError::Validation(format!(
                    "Group `{}` lists missing node `{missing}`.",
                    group.id
                )));
            }
        }
        for frame in self.layout.values() {
            let values = [Some(frame.x), Some(frame.y), frame.w, frame.h];
            if values
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e7)
                || frame.w.is_some_and(|w| w <= 0.0)
                || frame.h.is_some_and(|h| h <= 0.0)
            {
                return Err(AppError::Validation(
                    "Layout coordinates must be finite and sizes positive.".into(),
                ));
            }
        }
        Ok(())
    }
    /// Find a node by id or, failing that, by its (case-insensitive) label.
    pub fn resolve_node(&self, reference: &str) -> Result<&Node> {
        let reference = reference.trim();
        if let Some(node) = self.nodes.iter().find(|n| n.id == reference) {
            return Ok(node);
        }
        let matches: Vec<&Node> = self
            .nodes
            .iter()
            .filter(|n| n.label.eq_ignore_ascii_case(reference))
            .collect();
        match matches.as_slice() {
            [node] => Ok(node),
            [] => Err(AppError::Validation(format!(
                "No node `{reference}` in \"{}\". Nodes: {}",
                self.title,
                self.nodes
                    .iter()
                    .take(30)
                    .map(|n| n.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
            _ => Err(AppError::Validation(format!(
                "Several nodes are labelled `{reference}`; use one of their ids: {}",
                matches
                    .iter()
                    .map(|n| n.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
    /// Keep a replaced diagram's positions for nodes that survive the change.
    pub fn inherit_layout(&mut self, previous: &Diagram) {
        let ids: HashSet<String> = self.nodes.iter().map(|n| n.id.clone()).collect();
        for (id, frame) in &previous.layout {
            if ids.contains(id) && !self.layout.contains_key(id) {
                self.layout.insert(id.clone(), frame.clone());
            }
        }
    }
}

/// What a proposal changes, by label, for the review card and the agent's confirmation.
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub added_nodes: Vec<String>,
    pub changed_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub added_edges: Vec<String>,
    pub changed_edges: Vec<String>,
    pub removed_edges: Vec<String>,
}
impl Change {
    pub fn between(old: Option<&Diagram>, new: &Diagram) -> Change {
        let empty = Diagram::default();
        let old = old.unwrap_or(&empty);
        let old_nodes: HashMap<&str, &Node> =
            old.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let new_nodes: HashMap<&str, &Node> =
            new.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let label = |d: &Diagram, id: &str| {
            d.nodes
                .iter()
                .find(|n| n.id == id)
                .map_or(id.to_owned(), |n| n.label.clone())
        };
        let edge_label = |d: &Diagram, e: &Edge| {
            let mut text = format!("{} → {}", label(d, &e.source), label(d, &e.target));
            if let Some(l) = &e.label {
                text.push_str(&format!(" ({l})"));
            }
            text
        };
        let mut change = Change::default();
        for node in &new.nodes {
            match old_nodes.get(node.id.as_str()) {
                None => change.added_nodes.push(node.label.clone()),
                Some(previous) if *previous != node => {
                    change.changed_nodes.push(node.label.clone())
                }
                _ => {}
            }
        }
        for node in &old.nodes {
            if !new_nodes.contains_key(node.id.as_str()) {
                change.removed_nodes.push(node.label.clone());
            }
        }
        let old_edges: HashMap<&str, &Edge> =
            old.edges.iter().map(|e| (e.id.as_str(), e)).collect();
        let new_edges: HashSet<&str> = new.edges.iter().map(|e| e.id.as_str()).collect();
        for edge in &new.edges {
            match old_edges.get(edge.id.as_str()) {
                None => change.added_edges.push(edge_label(new, edge)),
                Some(previous) if *previous != edge => {
                    change.changed_edges.push(edge_label(new, edge))
                }
                _ => {}
            }
        }
        for edge in &old.edges {
            if !new_edges.contains(edge.id.as_str()) {
                change.removed_edges.push(edge_label(old, edge));
            }
        }
        change
    }
    pub fn summary(&self) -> String {
        format!(
            "+{} ~{} −{} nodes · +{} ~{} −{} edges",
            self.added_nodes.len(),
            self.changed_nodes.len(),
            self.removed_nodes.len(),
            self.added_edges.len(),
            self.changed_edges.len(),
            self.removed_edges.len()
        )
    }
}

/// Compact text for the agent: one line per node, edge and group, ids first.
pub fn render(slug: &str, diagram: &Diagram) -> String {
    let mut text = format!(
        "diagram {slug} [{}] \"{}\" — {} nodes, {} edges\n",
        diagram.kind,
        diagram.title,
        diagram.nodes.len(),
        diagram.edges.len()
    );
    if let Some(source) = &diagram.source {
        text.push_str(&format!("source: {}\n", source.name));
    }
    if let Some(description) = &diagram.description_md {
        text.push_str(description.trim());
        text.push('\n');
    }
    for node in &diagram.nodes {
        text.push_str(&format!(
            "node {} [{}] \"{}\"",
            node.id, node.kind, node.label
        ));
        if !node.tags.is_empty() {
            text.push_str(&format!(" #{}", node.tags.join(" #")));
        }
        if !node.code_refs.is_empty() {
            text.push_str(" refs: ");
            text.push_str(
                &node
                    .code_refs
                    .iter()
                    .map(CodeRef::display)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        text.push('\n');
        if let Some(body) = node
            .body_md
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            for line in body.lines() {
                text.push_str("  ");
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    for edge in &diagram.edges {
        text.push_str(&format!("edge {} -> {}", edge.source, edge.target));
        if let Some(label) = &edge.label {
            text.push_str(&format!(" \"{label}\""));
        }
        if let Some(kind) = &edge.kind {
            text.push_str(&format!(" [{kind}]"));
        }
        if !edge.code_refs.is_empty() {
            text.push_str(" refs: ");
            text.push_str(
                &edge
                    .code_refs
                    .iter()
                    .map(CodeRef::display)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
        }
        text.push('\n');
    }
    for group in &diagram.groups {
        text.push_str(&format!(
            "group {} \"{}\": {}\n",
            group.id,
            group.name,
            group.node_ids.join(", ")
        ));
    }
    text.truncate(text.trim_end().len());
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Diagram {
        serde_json::from_value(json!({
            "title": "Checkout",
            "nodes": [
                {"id":"api","kind":"service","label":"API","code_refs":["src/api.rs:10-20"]},
                {"id":"db","kind":"store","label":"Orders DB"}
            ],
            "edges": [{"source":"api","target":"db","label":"writes"}],
            "layout": {"api":{"x":1,"y":2},"ghost":{"x":0,"y":0}},
            "x-future": {"keep": true}
        }))
        .unwrap()
    }

    #[test]
    fn normalizes_and_round_trips_unknown_fields() {
        let mut diagram = sample();
        diagram.normalize();
        diagram.validate().unwrap();
        assert_eq!(diagram.edges[0].id, "api->db");
        assert!(!diagram.layout.contains_key("ghost"));
        let text = diagram.to_file().unwrap();
        assert!(text.contains("\"x-future\""), "{text}");
        assert!(text.contains("\"src/api.rs:10-20\""), "{text}");
        assert_eq!(Diagram::parse(&text).unwrap(), diagram);
    }

    #[test]
    fn rejects_broken_structure() {
        let mut dangling = sample();
        dangling.edges[0].target = "nowhere".into();
        dangling.normalize();
        assert!(dangling.validate().is_err());
        let mut twice = sample();
        twice.nodes[1].id = "api".into();
        assert!(twice.validate().is_err());
        let mut color = sample();
        color.nodes[0].style = Some(Style {
            color: Some("red".into()),
            shape: None,
        });
        assert!(color.validate().is_err());
        let mut schema = sample();
        schema.kind = "schema".into();
        assert!(schema.validate().is_err());
    }

    #[test]
    fn code_refs_stay_inside_the_repository() {
        for bad in [
            "../secret",
            "/etc/passwd",
            "a//b",
            "C:/x",
            "src/a.rs:5-2",
            "a\\b",
        ] {
            assert!(CodeRef::parse(bad).validate().is_err(), "{bad}");
        }
        let r = CodeRef::parse("./src/main.rs#L5-L20");
        assert_eq!(
            (r.path.as_str(), r.line, r.end_line),
            ("src/main.rs", Some(5), Some(20))
        );
        r.validate().unwrap();
        let object: CodeRef =
            serde_json::from_value(json!({"path":"src/a.rs","line":3,"symbol":"run"})).unwrap();
        assert_eq!(serde_json::to_value(&object).unwrap()["symbol"], "run");
        assert_eq!(CodeRef::parse("Makefile").display(), "Makefile");
    }

    #[test]
    fn slugs_and_change_summaries() {
        assert_eq!(slugify("Checkout flow (v2)"), "checkout-flow-v2");
        assert_eq!(slugify("!!!"), "diagram");
        assert!(valid_slug("../x").is_err());
        assert!(valid_slug("ok-1").is_ok());
        let old = sample();
        let mut new = sample();
        new.nodes[0].label = "Gateway".into();
        new.nodes.pop();
        new.edges.clear();
        new.nodes.push(Node {
            id: "cache".into(),
            kind: "store".into(),
            label: "Cache".into(),
            ..Node::default()
        });
        let change = Change::between(Some(&old), &new);
        assert_eq!(change.added_nodes, ["Cache"]);
        assert_eq!(change.changed_nodes, ["Gateway"]);
        assert_eq!(change.removed_nodes, ["Orders DB"]);
        assert_eq!(change.removed_edges.len(), 1);
        assert!(change.summary().starts_with("+1 ~1 −1 nodes"));
        let text = render("checkout", &sample());
        assert!(
            text.contains("node api [service] \"API\" refs: src/api.rs:10-20"),
            "{text}"
        );
        assert!(text.contains("edge api -> db \"writes\""), "{text}");
    }
}
