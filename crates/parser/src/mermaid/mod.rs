//! Mermaid: read with merman (Mermaid-compatible parsing and byte spans), annotate with
//! `%% @schematlas` lines, and edit by patching the original text.
mod families;
mod source;
mod write;

use crate::annotate::{self, Target};
use crate::error::{Error, Result};
use crate::model::{normalize_shape, Diagram, Edge, Group, Head, Node, Stroke};
use serde_json::Value;
use std::collections::HashMap;

pub use write::apply;

/// Read the `index`-th Mermaid block of a Markdown document.
pub fn parse_in_markdown(doc: &str, index: usize) -> Result<Document> {
    let block = crate::markdown::mermaid_blocks(doc)
        .into_iter()
        .nth(index)
        .ok_or_else(|| Error::InvalidEdit(format!("no Mermaid block #{index}")))?;
    parse(&block.source(doc))
}
/// Apply an edit to the `index`-th Mermaid block of a Markdown document; the rest of the
/// document is returned byte for byte.
pub fn apply_in_markdown(doc: &str, index: usize, edit: &crate::model::Edit) -> Result<String> {
    let block = crate::markdown::mermaid_blocks(doc)
        .into_iter()
        .nth(index)
        .ok_or_else(|| Error::InvalidEdit(format!("no Mermaid block #{index}")))?;
    let updated = apply(&block.source(doc), edit)?;
    Ok(crate::markdown::replace_block(doc, &block, &updated))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub diagram: Diagram,
}

/// Mermaid's diagram keywords that are not flowcharts, with a readable family name.
const OTHER_FAMILIES: [(&str, &str); 22] = [
    ("sequenceDiagram", "sequence"),
    ("classDiagram", "class"),
    ("stateDiagram", "state"),
    ("erDiagram", "entity relationship"),
    ("gantt", "gantt"),
    ("pie", "pie"),
    ("journey", "user journey"),
    ("gitGraph", "git graph"),
    ("mindmap", "mindmap"),
    ("timeline", "timeline"),
    ("quadrantChart", "quadrant"),
    ("requirementDiagram", "requirement"),
    ("C4Context", "C4"),
    ("C4Container", "C4"),
    ("C4Component", "C4"),
    ("sankey", "sankey"),
    ("xychart", "xy chart"),
    ("block", "block"),
    ("packet", "packet"),
    ("kanban", "kanban"),
    ("architecture", "architecture"),
    ("radar", "radar"),
];

/// The first word of the diagram, after front matter, comments and directives.
pub(crate) fn keyword(src: &str) -> Option<&str> {
    let body = source::after_front_matter(src);
    body.lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
        .and_then(|l| l.split(|c: char| c.is_whitespace() || c == ';').next())
}

fn front_matter_title(src: &str) -> Option<String> {
    let block = source::front_matter(src)?;
    block.lines().find_map(|line| {
        let title = line.strip_prefix("title:")?.trim();
        let title = title.trim_matches(|c| c == '"' || c == '\'').trim();
        (!title.is_empty()).then(|| title.to_owned())
    })
}

fn text(v: &Value) -> Option<String> {
    v.as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub(crate) fn engine() -> merman_core::Engine {
    merman_core::Engine::new()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Family {
    Flowchart,
    Class,
    Er,
    State,
}
impl Family {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Family::Flowchart => "flowchart",
            Family::Class => "class",
            Family::Er => "entity relationship",
            Family::State => "state",
        }
    }
}

/// Which graph-shaped family a source is, or a readable error for everything else.
pub(crate) fn family(src: &str) -> Result<Family> {
    match keyword(src) {
        Some("flowchart" | "graph" | "flowchart-elk") => Ok(Family::Flowchart),
        Some(w) if w.starts_with("classDiagram") => Ok(Family::Class),
        Some("erDiagram") => Ok(Family::Er),
        Some(w) if w.starts_with("stateDiagram") => Ok(Family::State),
        Some(word) => {
            if let Some((_, family)) = OTHER_FAMILIES.iter().find(|(k, _)| word.starts_with(k)) {
                return Err(Error::Unsupported((*family).to_owned()));
            }
            Err(Error::Parse {
                message: format!("`{word}` does not start a Mermaid diagram"),
                offset: None,
            })
        }
        None => Err(Error::Parse {
            message: "the diagram is empty".into(),
            offset: None,
        }),
    }
}

/// merman's model of a supported diagram, or a readable error.
pub(crate) fn merman_model(src: &str) -> Result<Value> {
    family(src)?;
    let parsed = engine()
        .parse_diagram_sync(src, merman_core::ParseOptions::strict())
        .map_err(|e| Error::Parse {
            message: e.to_string(),
            offset: None,
        })?
        .ok_or_else(|| Error::Parse {
            message: "no Mermaid diagram found".into(),
            offset: None,
        })?;
    Ok(parsed.model.as_ref().clone())
}

fn stroke_and_head(edge: &Value) -> (Stroke, Head, bool) {
    let stroke = match edge["stroke"].as_str() {
        Some("thick") => Stroke::Thick,
        Some("dotted") => Stroke::Dotted,
        Some("invisible") => Stroke::Invisible,
        _ => Stroke::Normal,
    };
    let kind = edge["type"].as_str().unwrap_or("arrow_point");
    let both = kind.starts_with("double_");
    let head = match kind.trim_start_matches("double_") {
        "arrow_circle" => Head::Circle,
        "arrow_cross" => Head::Cross,
        "arrow_open" => Head::None,
        _ => Head::Arrow,
    };
    (stroke, head, both)
}

/// Turn merman's flowchart model into ours.
fn convert(model: &Value) -> Diagram {
    let empty = vec![];
    // A link to a subgraph (`client --> backend`) makes merman list the subgraph as a node too;
    // it is the group, so edges may point at group ids.
    let subgraph_ids: Vec<&str> = model["subgraphs"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter_map(|s| s["id"].as_str())
        .collect();
    let nodes: Vec<Node> = model["nodes"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .filter(|n| {
            !n["id"]
                .as_str()
                .is_some_and(|id| subgraph_ids.contains(&id))
        })
        .map(|n| {
            let id = n["id"].as_str().unwrap_or_default().to_owned();
            let shape = n["shape"]
                .as_str()
                .or(n["layoutShape"].as_str())
                .and_then(normalize_shape);
            Node {
                label: text(&n["label"]).unwrap_or_else(|| id.clone()),
                shape,
                link: text(&n["link"]),
                // merman marks linked nodes `clickable`; that is rendering state, not the author's.
                classes: n["classes"]
                    .as_array()
                    .unwrap_or(&empty)
                    .iter()
                    .filter_map(|c| c.as_str())
                    .filter(|c| *c != "clickable")
                    .map(str::to_owned)
                    .collect(),
                id,
                ..Node::default()
            }
        })
        .collect();
    let edges = model["edges"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|e| {
            let (stroke, head, both_ends) = stroke_and_head(e);
            Edge {
                id: e["isUserDefinedId"]
                    .as_bool()
                    .unwrap_or(false)
                    .then(|| text(&e["id"]))
                    .flatten(),
                from: e["from"].as_str().unwrap_or_default().to_owned(),
                to: e["to"].as_str().unwrap_or_default().to_owned(),
                label: text(&e["label"]),
                stroke,
                head,
                both_ends,
                length: e["length"].as_u64().unwrap_or(1).clamp(1, 20) as u8,
                ..Edge::default()
            }
        })
        .collect();
    let order: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let subgraphs = model["subgraphs"].as_array().unwrap_or(&empty);
    let ids: Vec<&str> = subgraphs.iter().filter_map(|s| s["id"].as_str()).collect();
    let groups = subgraphs
        .iter()
        .map(|s| {
            let id = s["id"].as_str().unwrap_or_default().to_owned();
            let mut members: Vec<String> = s["nodes"]
                .as_array()
                .unwrap_or(&empty)
                .iter()
                .filter_map(|n| n.as_str())
                .filter(|n| order.contains_key(n) && !ids.contains(n))
                .map(str::to_owned)
                .collect();
            members.sort_by_key(|n| order[n.as_str()]);
            members.dedup();
            let parent = subgraphs
                .iter()
                .find(|p| {
                    p["nodes"]
                        .as_array()
                        .is_some_and(|ns| ns.iter().any(|n| n.as_str() == Some(id.as_str())))
                })
                .and_then(|p| p["id"].as_str())
                .map(str::to_owned);
            Group {
                title: text(&s["title"]).unwrap_or_else(|| id.clone()),
                nodes: members,
                parent,
                direction: text(&s["dir"]),
                id,
            }
        })
        .collect();
    Diagram {
        kind: "flowchart".into(),
        direction: text(&model["direction"]),
        title: None,
        nodes,
        edges,
        groups,
        annotation: Default::default(),
        layout: Default::default(),
    }
}

/// Read a Mermaid flowchart, class, entity-relationship or state diagram and its
/// `@schematlas` annotations.
pub fn parse(src: &str) -> Result<Document> {
    let model = merman_model(src)?;
    let mut diagram = match family(src)? {
        Family::Flowchart => convert(&model),
        Family::Class => families::class(&model),
        Family::Er => families::er(&model),
        Family::State => families::state(&model),
    };
    diagram.title = front_matter_title(src);
    // merman lists subgraphs innermost first; keep the order they are written in.
    let written: Vec<String> = source::layout(src)
        .statements
        .iter()
        .filter(|s| s.kind == source::Kind::Subgraph)
        .filter_map(|s| {
            let rest = src[s.range.clone()].strip_prefix("subgraph")?.trim_start();
            let id = rest.split(|c: char| c.is_whitespace() || c == '[').next()?;
            Some(id.trim_matches('"').to_owned())
        })
        .collect();
    diagram.groups.sort_by_key(|g| {
        written
            .iter()
            .position(|w| *w == g.id)
            .unwrap_or(usize::MAX)
    });
    for line in src.lines() {
        let Some((target, annotation)) = annotate::parse_line(line) else {
            continue;
        };
        match target {
            Target::Diagram => diagram.annotation = annotation,
            Target::Node(id) => {
                if let Some(node) = diagram.nodes.iter_mut().find(|n| n.id == id) {
                    node.annotation = annotation;
                }
            }
            Target::Edge(edge) => {
                if let Some(i) = edge.index(&diagram) {
                    diagram.edges[i].annotation = annotation;
                }
            }
        }
    }
    Ok(Document { diagram })
}
