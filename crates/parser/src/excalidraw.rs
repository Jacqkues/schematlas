//! Excalidraw scenes (`.excalidraw`): shapes with their bound text become nodes, arrows bound to
//! two shapes become edges, frames become groups. Schematlas annotations live in each element's
//! `customData.schematlas` and in a top-level `schematlas` key, which Excalidraw keeps and ignores.
//!
//! The drawing itself is edited in the Excalidraw editor, which writes the whole scene. Here only
//! annotations are written, and surgically: the rest of the file keeps its bytes.
use crate::annotate::Target;
use crate::error::{Error, Result};
use crate::model::{Annotation, Diagram, Edge, Group, Head, Node, Rect, Stroke};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::ops::Range;

pub const KIND: &str = "excalidraw";
pub(crate) const KEY: &str = "schematlas";

/// A new, empty scene.
pub fn empty() -> String {
    "{\n  \"type\": \"excalidraw\",\n  \"version\": 2,\n  \"source\": \"schematlas\",\n  \"elements\": [],\n  \"appState\": {},\n  \"files\": {}\n}\n".into()
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::Parse {
        message: message.into(),
        offset: None,
    }
}

pub(crate) fn scene(src: &str) -> Result<Map<String, Value>> {
    let value: Value = serde_json::from_str(src).map_err(|e| Error::Parse {
        message: format!("not valid JSON: {e}"),
        offset: None,
    })?;
    let Value::Object(scene) = value else {
        return Err(parse_error("an Excalidraw file is a JSON object"));
    };
    if scene.get("type").and_then(Value::as_str) != Some("excalidraw") {
        return Err(parse_error(
            "not an Excalidraw scene (`type` is not \"excalidraw\")",
        ));
    }
    Ok(scene)
}

pub(crate) fn live(scene: &Map<String, Value>) -> Vec<&Map<String, Value>> {
    scene
        .get("elements")
        .and_then(Value::as_array)
        .map(|elements| {
            elements
                .iter()
                .filter_map(Value::as_object)
                .filter(|e| e.get("isDeleted").and_then(Value::as_bool) != Some(true))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn text<'a>(e: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    e.get(key).and_then(Value::as_str)
}
pub(crate) fn number(e: &Map<String, Value>, key: &str) -> f64 {
    e.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}
pub(crate) fn binding<'a>(e: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    e.get(key)?.get("elementId")?.as_str()
}

fn annotation(value: Option<&Value>) -> Annotation {
    value
        .and_then(|v| v.get(KEY))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}

/// Shapes that can stand for a component. Free text is a node only when an arrow binds to it.
pub(crate) fn is_shape(kind: &str) -> bool {
    matches!(
        kind,
        "rectangle" | "ellipse" | "diamond" | "image" | "embeddable"
    )
}

/// Multi-line text in the model's label form.
pub(crate) fn label(text: &str) -> String {
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("<br>")
}

/// The scene as a graph, with each shape's position and size.
pub fn parse(src: &str) -> Result<Diagram> {
    let scene = scene(src)?;
    let elements = live(&scene);
    let mut labels: HashMap<&str, &str> = HashMap::new();
    for e in &elements {
        if let (Some("text"), Some(container), Some(t)) =
            (text(e, "type"), text(e, "containerId"), text(e, "text"))
        {
            labels.insert(container, t);
        }
    }
    let bound: Vec<&str> = elements
        .iter()
        .filter(|e| text(e, "type") == Some("arrow"))
        .flat_map(|e| [binding(e, "startBinding"), binding(e, "endBinding")])
        .flatten()
        .collect();

    let mut d = Diagram {
        kind: KIND.into(),
        annotation: annotation(Some(&Value::Object(scene.clone()))),
        ..Diagram::default()
    };
    d.title = d.annotation.title.clone();
    for e in &elements {
        let (Some(kind), Some(id)) = (text(e, "type"), text(e, "id")) else {
            continue;
        };
        let free_text = kind == "text" && text(e, "containerId").is_none() && bound.contains(&id);
        if !is_shape(kind) && !free_text {
            continue;
        }
        let shown = if free_text {
            text(e, "text")
        } else {
            labels.get(id).copied()
        };
        d.nodes.push(Node {
            id: id.to_owned(),
            label: shown.map(label).unwrap_or_default(),
            shape: match kind {
                "ellipse" => Some("circle".into()),
                "diamond" => Some("diamond".into()),
                "rectangle" if e.get("roundness").is_some_and(|r| !r.is_null()) => {
                    Some("rounded".into())
                }
                _ => None,
            },
            link: text(e, "link").map(str::to_owned),
            annotation: annotation(e.get("customData")),
            ..Node::default()
        });
        d.layout.insert(
            id.to_owned(),
            Rect {
                x: number(e, "x"),
                y: number(e, "y"),
                width: number(e, "width"),
                height: number(e, "height"),
            },
        );
    }
    let is_node = |id: &str| d.nodes.iter().any(|n| n.id == id);
    let mut edges = vec![];
    for e in &elements {
        if text(e, "type") != Some("arrow") {
            continue;
        }
        let (Some(from), Some(to), Some(id)) = (
            binding(e, "startBinding"),
            binding(e, "endBinding"),
            text(e, "id"),
        ) else {
            continue;
        };
        if !is_node(from) || !is_node(to) {
            continue;
        }
        let head = |key: &str| match text(e, key) {
            None => Head::None,
            Some("dot" | "circle" | "circle_outline") => Head::Circle,
            Some("bar") => Head::Cross,
            Some("triangle" | "triangle_outline") => Head::Triangle,
            Some("diamond_outline") => Head::Diamond,
            Some("diamond") => Head::FilledDiamond,
            Some(_) => Head::Arrow,
        };
        let end = head("endArrowhead");
        edges.push(Edge {
            id: Some(id.to_owned()),
            from: from.to_owned(),
            to: to.to_owned(),
            label: labels.get(id).map(|t| label(t)).filter(|l| !l.is_empty()),
            stroke: match text(e, "strokeStyle") {
                Some("dashed" | "dotted") => Stroke::Dotted,
                _ if number(e, "strokeWidth") >= 4.0 => Stroke::Thick,
                _ => Stroke::Normal,
            },
            head: end,
            both_ends: end != Head::None && head("startArrowhead") != Head::None,
            annotation: annotation(e.get("customData")),
            ..Edge::default()
        });
    }
    d.edges = edges;
    for e in &elements {
        if !matches!(text(e, "type"), Some("frame" | "magicframe")) {
            continue;
        }
        let Some(id) = text(e, "id") else { continue };
        let members = elements
            .iter()
            .filter(|m| text(m, "frameId") == Some(id))
            .filter_map(|m| text(m, "id"))
            .filter(|m| is_node(m))
            .map(str::to_owned)
            .collect();
        d.groups.push(Group {
            id: id.to_owned(),
            title: text(e, "name").unwrap_or("Frame").to_owned(),
            nodes: members,
            ..Group::default()
        });
        d.layout.insert(
            id.to_owned(),
            Rect {
                x: number(e, "x"),
                y: number(e, "y"),
                width: number(e, "width"),
                height: number(e, "height"),
            },
        );
    }
    Ok(d)
}

// ---------- Surgical writes ----------

/// One `"key": value` of a JSON object, by byte ranges in the source.
struct Member {
    key: String,
    /// From the opening quote of the key to the end of the value.
    whole: Range<usize>,
    value: Range<usize>,
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}
fn end_of_string(b: &[u8], mut i: usize) -> Result<usize> {
    i += 1;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => return Ok(i + 1),
            _ => i += 1,
        }
    }
    Err(parse_error("unterminated string"))
}
fn end_of_value(b: &[u8], i: usize) -> Result<usize> {
    match b.get(i) {
        Some(b'"') => end_of_string(b, i),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut j = i;
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        j = end_of_string(b, j)?;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            Err(parse_error("unterminated object or array"))
        }
        Some(_) => {
            let mut j = i;
            while j < b.len() && !matches!(b[j], b',' | b'}' | b']') && !b[j].is_ascii_whitespace()
            {
                j += 1;
            }
            Ok(j)
        }
        None => Err(parse_error("unexpected end of file")),
    }
}
/// The members of the object starting at `start` (its `{`), and the index of its `}`.
fn members(src: &str, start: usize) -> Result<(Vec<Member>, usize)> {
    let b = src.as_bytes();
    let mut out = vec![];
    let mut i = skip_ws(b, start + 1);
    loop {
        match b.get(i) {
            Some(b'}') => return Ok((out, i)),
            Some(b'"') => {
                let key_end = end_of_string(b, i)?;
                let key: String = serde_json::from_str(&src[i..key_end])
                    .map_err(|e| parse_error(e.to_string()))?;
                let colon = skip_ws(b, key_end);
                if b.get(colon) != Some(&b':') {
                    return Err(parse_error("expected `:`"));
                }
                let value_start = skip_ws(b, colon + 1);
                let value_end = end_of_value(b, value_start)?;
                out.push(Member {
                    key,
                    whole: i..value_end,
                    value: value_start..value_end,
                });
                i = skip_ws(b, value_end);
                if b.get(i) == Some(&b',') {
                    i = skip_ws(b, i + 1);
                }
            }
            _ => return Err(parse_error("expected a key or `}`")),
        }
    }
}
/// The byte ranges of the elements of the array starting at `start` (its `[`).
fn items(src: &str, start: usize) -> Result<Vec<Range<usize>>> {
    let b = src.as_bytes();
    let mut out = vec![];
    let mut i = skip_ws(b, start + 1);
    while b.get(i) != Some(&b']') {
        let end = end_of_value(b, i)?;
        out.push(i..end);
        i = skip_ws(b, end);
        if b.get(i) == Some(&b',') {
            i = skip_ws(b, i + 1);
        }
        if i >= b.len() {
            return Err(parse_error("unterminated array"));
        }
    }
    Ok(out)
}

/// The indentation of the line holding byte `at`.
fn indent_at(src: &str, at: usize) -> &str {
    let line = src[..at].rfind('\n').map_or(0, |n| n + 1);
    let rest = &src[line..];
    &rest[..rest.len() - rest.trim_start_matches([' ', '\t']).len()]
}

/// Set (or, when `value` is None, remove) `key` in the object starting at `start`, touching
/// nothing else. A new member is written last, indented like its siblings.
fn set_member(src: &str, start: usize, key: &str, value: Option<&Value>) -> Result<String> {
    let (members, close) = members(src, start)?;
    let found = members.iter().position(|m| m.key == key);
    let compact = !src[start..close].contains('\n');
    let pad = members
        .first()
        .map(|m| indent_at(src, m.whole.start).to_owned())
        .unwrap_or_else(|| format!("{}  ", indent_at(src, start)));
    let render = |v: &Value| -> Result<String> {
        let text = if compact {
            serde_json::to_string(v)
        } else {
            serde_json::to_string_pretty(v)
        }
        .map_err(|e| parse_error(e.to_string()))?;
        // Continuation lines follow the member's indentation.
        Ok(text.replace('\n', &format!("\n{pad}")))
    };
    let mut out = String::with_capacity(src.len() + 64);
    match (found, value) {
        (Some(i), Some(v)) => {
            let m = &members[i];
            out.push_str(&src[..m.value.start]);
            out.push_str(&render(v)?);
            out.push_str(&src[m.value.end..]);
        }
        (Some(i), None) => {
            let m = &members[i];
            // Remove the member with the separator on one side.
            let (from, to) = if i + 1 < members.len() {
                (m.whole.start, members[i + 1].whole.start)
            } else if i > 0 {
                (members[i - 1].whole.end, m.whole.end)
            } else {
                (start + 1, close)
            };
            out.push_str(&src[..from]);
            out.push_str(&src[to..]);
        }
        (None, Some(v)) => {
            let member = format!("{}: {}", Value::String(key.into()), render(v)?);
            match members.last() {
                Some(last) => {
                    out.push_str(&src[..last.whole.end]);
                    if compact {
                        out.push_str(", ");
                    } else {
                        out.push_str(&format!(",\n{pad}"));
                    }
                    out.push_str(&member);
                    out.push_str(&src[last.whole.end..]);
                }
                None => {
                    out.push_str(&src[..start + 1]);
                    if compact {
                        out.push_str(&member);
                    } else {
                        out.push_str(&format!("\n{pad}{member}\n{}", indent_at(src, start)));
                    }
                    out.push_str(&src[close..]);
                }
            }
        }
        (None, None) => out.push_str(src),
    }
    Ok(out)
}

fn annotation_value(annotation: &Annotation) -> Result<Option<Value>> {
    if annotation.is_empty() {
        return Ok(None);
    }
    for code in &annotation.code {
        code.validate()?;
    }
    serde_json::to_value(annotation)
        .map(Some)
        .map_err(|e| Error::InvalidEdit(e.to_string()))
}

/// Replace the annotation of the scene, a shape or an arrow; an empty one removes it.
pub fn annotate(src: &str, target: &Target, annotation: &Annotation) -> Result<String> {
    let value = annotation_value(annotation)?;
    let start = src
        .find('{')
        .ok_or_else(|| parse_error("an Excalidraw file is a JSON object"))?;
    let element = match target {
        Target::Diagram => return set_member(src, start, KEY, value.as_ref()),
        Target::Node(id) => {
            let d = parse(src)?;
            if !d.nodes.iter().any(|n| &n.id == id) {
                return Err(Error::InvalidEdit(format!("no shape `{id}`")));
            }
            id.clone()
        }
        Target::Edge(edge) => {
            let d = parse(src)?;
            let index = edge.index(&d).ok_or_else(|| {
                Error::InvalidEdit(format!("no arrow from `{}` to `{}`", edge.from, edge.to))
            })?;
            d.edges[index].id.clone().unwrap_or_default()
        }
    };
    let (top, _) = members(src, start)?;
    let elements = top
        .iter()
        .find(|m| m.key == "elements")
        .ok_or_else(|| parse_error("the scene has no `elements`"))?;
    for item in items(src, elements.value.start)? {
        let object: Value =
            serde_json::from_str(&src[item.clone()]).map_err(|e| parse_error(e.to_string()))?;
        if object.get("id").and_then(Value::as_str) != Some(element.as_str())
            || object.get("isDeleted").and_then(Value::as_bool) == Some(true)
        {
            continue;
        }
        let mut custom = object
            .get("customData")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        match &value {
            Some(v) => {
                custom.insert(KEY.into(), v.clone());
            }
            None => {
                custom.remove(KEY);
            }
        }
        let custom = (!custom.is_empty()).then_some(Value::Object(custom));
        return set_member(src, item.start, "customData", custom.as_ref());
    }
    Err(Error::InvalidEdit(format!("no element `{element}`")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CodeRef, EdgeRef};

    const SCENE: &str = r##"{
  "type": "excalidraw",
  "version": 2,
  "source": "https://excalidraw.com",
  "elements": [
    {
      "id": "api",
      "type": "rectangle",
      "x": 100,
      "y": 50,
      "width": 160,
      "height": 80,
      "roundness": { "type": 3 },
      "boundElements": [{ "type": "text", "id": "api-label" }]
    },
    {
      "id": "api-label",
      "type": "text",
      "x": 120,
      "y": 70,
      "width": 40,
      "height": 25,
      "text": "API\nserver",
      "containerId": "api"
    },
    {
      "id": "db",
      "type": "ellipse",
      "x": 400,
      "y": 50,
      "width": 120,
      "height": 80,
      "customData": { "color": "blue" }
    },
    {
      "id": "arrow",
      "type": "arrow",
      "x": 260,
      "y": 90,
      "width": 140,
      "height": 0,
      "strokeStyle": "dashed",
      "startBinding": { "elementId": "api", "focus": 0, "gap": 1 },
      "endBinding": { "elementId": "db", "focus": 0, "gap": 1 },
      "startArrowhead": null,
      "endArrowhead": "arrow"
    },
    {
      "id": "gone",
      "type": "rectangle",
      "x": 0,
      "y": 0,
      "width": 10,
      "height": 10,
      "isDeleted": true
    },
    {
      "id": "frame",
      "type": "frame",
      "name": "Backend",
      "x": 80,
      "y": 20,
      "width": 480,
      "height": 140
    }
  ],
  "appState": { "viewBackgroundColor": "#ffffff" },
  "files": {}
}
"##;

    #[test]
    fn shapes_become_nodes_and_bound_arrows_edges() {
        let d = parse(SCENE).unwrap();
        assert_eq!(d.kind, "excalidraw");
        let ids: Vec<_> = d.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["api", "db"]);
        assert_eq!(d.nodes[0].label, "API<br>server");
        assert_eq!(d.nodes[0].shape.as_deref(), Some("rounded"));
        assert_eq!(d.nodes[1].shape.as_deref(), Some("circle"));
        assert_eq!(d.edges.len(), 1);
        assert_eq!(
            (d.edges[0].from.as_str(), d.edges[0].to.as_str()),
            ("api", "db")
        );
        assert_eq!(d.edges[0].stroke, Stroke::Dotted);
        assert!(!d.edges[0].both_ends);
        assert_eq!(d.layout["api"].width, 160.0);
        assert!(d.groups.iter().any(|g| g.title == "Backend"));
    }

    #[test]
    fn annotations_are_written_in_place_and_read_back() {
        let note = Annotation {
            note: Some("Serves the \"cart\"".into()),
            code: vec![CodeRef::parse("src/api.rs:10-40")],
            ..Annotation::default()
        };
        let out = annotate(SCENE, &Target::Node("api".into()), &note).unwrap();
        let d = parse(&out).unwrap();
        assert_eq!(d.nodes[0].annotation, note);
        // Everything else keeps its bytes: only lines were added.
        let removed = SCENE.lines().filter(|l| !out.contains(l)).count();
        assert_eq!(removed, 0, "{out}");
        // Other custom data survives, and removing the annotation restores the file.
        let db = annotate(SCENE, &Target::Node("db".into()), &note).unwrap();
        assert!(db.contains("\"color\": \"blue\""));
        assert_eq!(parse(&db).unwrap().nodes[1].annotation, note);
        let back = annotate(&db, &Target::Node("db".into()), &Annotation::default()).unwrap();
        assert_eq!(
            parse(&back).unwrap().nodes[1].annotation,
            Annotation::default()
        );
        assert!(back.contains("\"color\": \"blue\""));
        let cleared = annotate(&out, &Target::Node("api".into()), &Annotation::default()).unwrap();
        assert_eq!(cleared, SCENE);
    }

    #[test]
    fn edges_and_the_scene_take_annotations() {
        let note = Annotation {
            note: Some("Reads orders".into()),
            ..Annotation::default()
        };
        let edge = EdgeRef {
            from: "api".into(),
            to: "db".into(),
            nth: 0,
        };
        let out = annotate(SCENE, &Target::Edge(edge), &note).unwrap();
        assert_eq!(parse(&out).unwrap().edges[0].annotation, note);
        let titled = Annotation {
            title: Some("Checkout".into()),
            ..Annotation::default()
        };
        let out = annotate(&out, &Target::Diagram, &titled).unwrap();
        let d = parse(&out).unwrap();
        assert_eq!(d.title.as_deref(), Some("Checkout"));
        assert_eq!(d.edges[0].annotation, note);
    }

    #[test]
    fn compact_files_stay_compact_and_bad_input_is_refused() {
        let compact =
            serde_json::to_string(&serde_json::from_str::<Value>(SCENE).unwrap()).unwrap();
        let note = Annotation {
            kind: Some("service".into()),
            ..Annotation::default()
        };
        let out = annotate(&compact, &Target::Node("api".into()), &note).unwrap();
        assert!(!out.contains('\n'));
        assert_eq!(parse(&out).unwrap().nodes[0].annotation, note);
        assert!(parse("{\"type\":\"other\"}").is_err());
        assert!(parse("not json").is_err());
        assert!(annotate(SCENE, &Target::Node("missing".into()), &note).is_err());
        let escaping = Annotation {
            code: vec![CodeRef::parse("../secret")],
            ..Annotation::default()
        };
        assert!(annotate(SCENE, &Target::Node("api".into()), &escaping).is_err());
        assert!(parse(&empty()).unwrap().nodes.is_empty());
    }
}
