//! The format-neutral diagram every reader produces, and the edits every writer understands.
//!
//! `apply` defines what an edit means on the model. Each writer must produce text that reads back
//! as exactly `apply(read(text), edit)`; the test suites hold them to that.
use crate::annotate::Target;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Diagram {
    /// `flowchart` today; other Mermaid families and formats later.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// From the file's own front matter, as Mermaid shows it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<Group>,
    #[serde(skip_serializing_if = "Annotation::is_empty")]
    pub annotation: Annotation,
    /// Absolute positions and sizes of nodes and groups, for formats that store them (draw.io).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub layout: BTreeMap<String, Rect>,
    /// A sequence diagram's steps in reading order. Its participants are `nodes` (so they take
    /// annotations) and its messages are also `edges`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<Sequence>,
}

/// The time-ordered part of a sequence diagram.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sequence {
    /// Messages are numbered (`autonumber`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub autonumber: bool,
    pub steps: Vec<Step>,
}

/// One line of a sequence diagram, top to bottom.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "camelCase")]
pub enum Step {
    Message {
        from: String,
        to: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        label: String,
        /// `dotted` for replies (`-->>`), else solid.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dotted: bool,
        arrow: Arrow,
    },
    Note {
        /// One participant, or the two a note spans.
        over: Vec<String>,
        placement: NotePlacement,
        text: String,
    },
    Activate {
        actor: String,
    },
    Deactivate {
        actor: String,
    },
    /// `loop`, `alt`, `opt`, `par`, `critical`, `break` or `rect` opens a frame.
    Start {
        block: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        label: String,
    },
    /// `else`, `and` or `option` divides the open frame.
    Branch {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        label: String,
    },
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Arrow {
    /// `->>`: a filled head.
    Filled,
    /// `->`: a line without a head.
    Open,
    /// `-x`: a cross.
    Cross,
    /// `-)`: an open, asynchronous head.
    Async,
    /// `<<->>`: heads at both ends.
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotePlacement {
    Over,
    LeftOf,
    RightOf,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// draw.io diagrams store positions and give every edge a cell id.
pub const DRAWIO: &str = "drawio";
pub const NEW_NODE_SIZE: (f64, f64) = (120.0, 60.0);

impl Diagram {
    fn positioned(&self) -> bool {
        self.kind == DRAWIO
    }
    fn id_taken(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
            || self.groups.iter().any(|g| g.id == id)
            || self.edges.iter().any(|e| e.id.as_deref() == Some(id))
    }
    /// A deterministic, unused id for a new edge between two cells (draw.io needs one).
    pub fn new_edge_id(&self, from: &str, to: &str) -> String {
        let base = format!("{from}-{to}");
        let mut id = base.clone();
        let mut n = 2;
        while self.id_taken(&id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }
    /// Where a new node goes: right of everything, aligned with the top.
    pub fn free_spot(&self) -> Rect {
        let right = self
            .layout
            .values()
            .map(|r| r.x + r.width)
            .fold(f64::NEG_INFINITY, f64::max);
        let top = self
            .layout
            .values()
            .map(|r| r.y)
            .fold(f64::INFINITY, f64::min);
        Rect {
            x: if right.is_finite() {
                right + 40.0
            } else {
                40.0
            },
            y: if top.is_finite() { top } else { 40.0 },
            width: NEW_NODE_SIZE.0,
            height: NEW_NODE_SIZE.1,
        }
    }
    /// A group and every group nested in it, at any depth.
    pub fn group_tree(&self, id: &str) -> Vec<String> {
        let mut out = vec![id.to_owned()];
        let mut i = 0;
        while i < out.len() {
            let parent = out[i].clone();
            out.extend(
                self.groups
                    .iter()
                    .filter(|g| g.parent.as_deref() == Some(&parent))
                    .map(|g| g.id.clone()),
            );
            i += 1;
        }
        out
    }
}

impl Diagram {
    /// The same diagram, ignoring node order. Mermaid orders nodes by how they were created
    /// (nodes only named in `style` lines come last), which carries no meaning; edge order does
    /// (`linkStyle` and edge references count edges), so it stays significant.
    pub fn same_as(&self, other: &Diagram) -> bool {
        self.canonical() == other.canonical()
    }
    fn canonical(&self) -> Diagram {
        let mut d = self.clone();
        d.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        for group in &mut d.groups {
            group.nodes.sort();
        }
        d
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Node {
    pub id: String,
    pub label: String,
    /// None is the default rectangle. Otherwise a canonical name such as `cylinder` or `diamond`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shape: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<String>,
    /// Lines shown inside the box: class members and methods, entity attributes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<String>,
    #[serde(skip_serializing_if = "Annotation::is_empty")]
    pub annotation: Annotation,
}
impl Node {
    /// What the node represents: the annotated kind, else a guess from its shape.
    pub fn kind(&self) -> &str {
        if let Some(kind) = &self.annotation.kind {
            return kind;
        }
        match self.shape.as_deref() {
            Some("cylinder") => "store",
            Some("diamond") => "decision",
            Some("circle" | "doublecircle") => "actor",
            Some("asymmetric") => "event",
            Some("actor") => "actor",
            Some("queue") => "queue",
            Some("subroutine") => "module",
            _ => "node",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stroke {
    #[default]
    Normal,
    Thick,
    Dotted,
    Invisible,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Head {
    #[default]
    Arrow,
    Circle,
    Cross,
    None,
    /// UML inheritance.
    Triangle,
    /// UML aggregation.
    Diamond,
    /// UML composition.
    FilledDiamond,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Edge {
    /// Only ids the author wrote (`a e1@--> b`); generated ids are not part of the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub stroke: Stroke,
    pub head: Head,
    /// `<-->`, `o--o`, `x--x`: the head is drawn at both ends.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub both_ends: bool,
    /// Extra length (`---->` is 3).
    pub length: u8,
    /// Text at the source end, such as an ER cardinality (`1`, `0..n`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tail_label: Option<String>,
    /// Text at the target end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_label: Option<String>,
    #[serde(skip_serializing_if = "Annotation::is_empty")]
    pub annotation: Annotation,
}
impl Default for Edge {
    fn default() -> Self {
        Edge {
            id: None,
            from: String::new(),
            to: String::new(),
            label: None,
            stroke: Stroke::Normal,
            head: Head::Arrow,
            both_ends: false,
            length: 1,
            tail_label: None,
            head_label: None,
            annotation: Annotation::default(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    pub id: String,
    pub title: String,
    /// Direct member nodes, in diagram order. Nested groups point at their parent instead.
    pub nodes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
}

/// What Schematlas adds to a diagram: evidence in the code, notes, kinds and tags. Stored inside
/// the diagram file in a way the original tool ignores.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Annotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub code: Vec<CodeRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Keys this version does not know, kept so newer files survive older apps.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, String>,
}
impl Annotation {
    pub fn is_empty(&self) -> bool {
        *self == Annotation::default()
    }
}

/// A repository location: `path`, `path:12` or `path:12-40`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeRef {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u32>,
}
impl CodeRef {
    /// Accepts `path:5`, `path:5-20` and the Markdown-link form `path#L5-L20`.
    pub fn parse(text: &str) -> CodeRef {
        let text = text.trim();
        let text = text.strip_prefix("./").unwrap_or(text);
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
        }
    }
    /// Relative, inside the repository, with a sensible line range.
    pub fn validate(&self) -> Result<()> {
        let path = &self.path;
        let bad = path.is_empty()
            || path.len() > 512
            || path.starts_with('/')
            || path.contains('\\')
            || path.contains('\0')
            || path.chars().nth(1) == Some(':')
            || path.split('/').any(|part| part == ".." || part.is_empty());
        if bad {
            return Err(Error::InvalidEdit(format!(
                "code reference `{path}` must be a relative path inside the repository"
            )));
        }
        if matches!((self.line, self.end_line), (Some(0), _) | (_, Some(0)))
            || matches!((self.line, self.end_line), (Some(a), Some(b)) if b < a)
            || (self.line.is_none() && self.end_line.is_some())
        {
            return Err(Error::InvalidEdit(format!(
                "code reference `{self}` has an invalid line range"
            )));
        }
        Ok(())
    }
}
impl fmt::Display for CodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.path)?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
            if let Some(end) = self.end_line.filter(|e| *e != line) {
                write!(f, "-{end}")?;
            }
        }
        Ok(())
    }
}

/// An edge by its endpoints and its position among edges with the same endpoints.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeRef {
    pub from: String,
    pub to: String,
    pub nth: usize,
}
impl EdgeRef {
    /// Index of the referenced edge in `diagram.edges`.
    pub fn index(&self, diagram: &Diagram) -> Option<usize> {
        diagram
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.from == self.from && e.to == self.to)
            .nth(self.nth)
            .map(|(i, _)| i)
    }
    /// The reference for `diagram.edges[index]`.
    pub fn of(diagram: &Diagram, index: usize) -> EdgeRef {
        let edge = &diagram.edges[index];
        let nth = diagram.edges[..index]
            .iter()
            .filter(|e| e.from == edge.from && e.to == edge.to)
            .count();
        EdgeRef {
            from: edge.from.clone(),
            to: edge.to.clone(),
            nth,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Edit {
    AddNode {
        id: String,
        label: String,
        shape: Option<String>,
    },
    SetLabel {
        id: String,
        label: String,
    },
    RemoveNode {
        id: String,
    },
    AddEdge {
        from: String,
        to: String,
        label: Option<String>,
    },
    RemoveEdge(EdgeRef),
    SetEdgeLabel {
        edge: EdgeRef,
        label: Option<String>,
    },
    /// Replaces the whole annotation; an empty one removes it.
    Annotate {
        target: Target,
        annotation: Annotation,
    },
    /// Move a node or a group (with everything inside it) to an absolute position. Only formats
    /// that store positions accept it.
    Move {
        id: String,
        x: f64,
        y: f64,
    },
}

const KEYWORDS: [&str; 11] = [
    "end",
    "graph",
    "flowchart",
    "subgraph",
    "direction",
    "class",
    "classDef",
    "style",
    "linkStyle",
    "click",
    "default",
];
/// Ids a writer can emit without quoting: letters, digits, `_`, `-` and `.`, not a keyword.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
        && !id.starts_with(['-', '.'])
        && !KEYWORDS.contains(&id)
}

/// Canonical shape names, so `[(x)]` and `@{ shape: cyl }` mean the same thing.
pub fn normalize_shape(shape: &str) -> Option<String> {
    let canonical = match shape {
        "" | "square" | "squareRect" | "rect" | "rectangle" | "proc" | "process" => return None,
        "round" | "roundedRect" | "rounded" | "event" => "rounded",
        "cyl" | "cylinder" | "db" | "database" => "cylinder",
        "diamond" | "diam" | "decision" | "question" => "diamond",
        "circle" | "circ" => "circle",
        "doublecircle" | "dbl-circ" | "double-circle" => "doublecircle",
        "odd" | "asymmetric" => "asymmetric",
        "hexagon" | "hex" | "prepare" => "hexagon",
        "subroutine" | "fr-rect" | "subproc" | "framed-rectangle" => "subroutine",
        "lean_right" | "lean-r" | "lean-right" => "lean_right",
        "lean_left" | "lean-l" | "lean-left" => "lean_left",
        "trapezoid" | "trap-b" => "trapezoid",
        "inv_trapezoid" | "trap-t" => "inv_trapezoid",
        "stadium" | "pill" | "terminal" => "stadium",
        other => other,
    };
    Some(canonical.to_owned())
}

fn node_index(d: &Diagram, id: &str) -> Result<usize> {
    d.nodes
        .iter()
        .position(|n| n.id == id)
        .ok_or_else(|| Error::InvalidEdit(format!("no node `{id}`")))
}
fn edge_index(d: &Diagram, edge: &EdgeRef) -> Result<usize> {
    edge.index(d).ok_or_else(|| {
        Error::InvalidEdit(format!(
            "no edge {} --> {} (#{})",
            edge.from, edge.to, edge.nth
        ))
    })
}
fn clean_label(label: &str) -> Result<String> {
    let label = label.trim();
    if label.is_empty() || label.chars().count() > 500 || label.contains(['\n', '\r']) {
        return Err(Error::InvalidEdit(
            "labels are one line of 1 to 500 characters".into(),
        ));
    }
    Ok(label.to_owned())
}
fn clean_edge_label(label: &Option<String>) -> Result<Option<String>> {
    match label.as_deref().map(str::trim) {
        None | Some("") => Ok(None),
        Some(label) => clean_label(label).map(Some),
    }
}

/// The meaning of an edit on the model. Writers implement the same thing on text.
pub fn apply(diagram: &Diagram, edit: &Edit) -> Result<Diagram> {
    let mut d = diagram.clone();
    match edit {
        Edit::AddNode { id, label, shape } => {
            if !valid_id(id) {
                return Err(Error::InvalidEdit(format!(
                    "`{id}` is not a usable node id"
                )));
            }
            if d.nodes.iter().any(|n| &n.id == id) || (d.positioned() && d.id_taken(id)) {
                return Err(Error::InvalidEdit(format!("`{id}` is already used")));
            }
            let label = clean_label(label)?;
            if d.positioned() {
                let spot = d.free_spot();
                d.layout.insert(id.clone(), spot);
            }
            d.nodes.push(Node {
                id: id.clone(),
                label,
                shape: shape.as_deref().and_then(normalize_shape),
                ..Node::default()
            });
        }
        Edit::SetLabel { id, label } => {
            let i = node_index(&d, id)?;
            d.nodes[i].label = clean_label(label)?;
        }
        Edit::RemoveNode { id } => {
            let i = node_index(&d, id)?;
            d.nodes.remove(i);
            d.edges.retain(|e| &e.from != id && &e.to != id);
            for group in &mut d.groups {
                group.nodes.retain(|n| n != id);
            }
            d.layout.remove(id);
        }
        Edit::AddEdge { from, to, label } => {
            // Either end may be a node or a group (Mermaid links to subgraphs by id).
            for end in [from, to] {
                if !d.nodes.iter().any(|n| &n.id == end) && !d.groups.iter().any(|g| &g.id == end) {
                    return Err(Error::InvalidEdit(format!("no node or group `{end}`")));
                }
            }
            let id = d.positioned().then(|| d.new_edge_id(from, to));
            d.edges.push(Edge {
                id,
                from: from.clone(),
                to: to.clone(),
                label: clean_edge_label(label)?,
                ..Edge::default()
            });
        }
        Edit::RemoveEdge(edge) => {
            let i = edge_index(&d, edge)?;
            d.edges.remove(i);
        }
        Edit::SetEdgeLabel { edge, label } => {
            let i = edge_index(&d, edge)?;
            let label = clean_edge_label(label)?;
            if label.is_some() && d.edges[i].stroke == Stroke::Invisible {
                return Err(Error::InvalidEdit(
                    "invisible links cannot carry a label".into(),
                ));
            }
            d.edges[i].label = label;
        }
        Edit::Annotate { target, annotation } => {
            for code in &annotation.code {
                code.validate()?;
            }
            match target {
                Target::Diagram => d.annotation = annotation.clone(),
                Target::Node(id) => {
                    let i = node_index(&d, id)?;
                    d.nodes[i].annotation = annotation.clone();
                }
                Target::Edge(edge) => {
                    let i = edge_index(&d, edge)?;
                    d.edges[i].annotation = annotation.clone();
                }
            }
        }
        Edit::Move { id, x, y } => {
            if !d.positioned() {
                return Err(Error::InvalidEdit(
                    "this format does not store positions; layout is kept beside the file".into(),
                ));
            }
            if !x.is_finite() || !y.is_finite() || x.abs() > 1e7 || y.abs() > 1e7 {
                return Err(Error::InvalidEdit("positions must be finite".into()));
            }
            let is_group = d.groups.iter().any(|g| &g.id == id);
            if !is_group {
                node_index(&d, id)?;
            }
            let current = d.layout.get(id).copied().unwrap_or(Rect {
                x: 0.0,
                y: 0.0,
                width: NEW_NODE_SIZE.0,
                height: NEW_NODE_SIZE.1,
            });
            let (dx, dy) = (x - current.x, y - current.y);
            d.layout.insert(
                id.clone(),
                Rect {
                    x: *x,
                    y: *y,
                    ..current
                },
            );
            if is_group {
                // A container carries its contents along.
                let tree = d.group_tree(id);
                let mut inside: Vec<String> = tree[1..].to_vec();
                for g in &tree {
                    if let Some(group) = d.groups.iter().find(|x| &x.id == g) {
                        inside.extend(group.nodes.iter().cloned());
                    }
                }
                for member in inside {
                    if let Some(r) = d.layout.get_mut(&member) {
                        r.x += dx;
                        r.y += dy;
                    }
                }
            }
        }
    }
    Ok(d)
}
