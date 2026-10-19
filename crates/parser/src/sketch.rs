//! An Excalidraw sketch kept in step with a diagram (its *source*, usually a Mermaid file).
//!
//! The source holds the structure — which components, which connections, their names — and the
//! sketch holds the looks and whatever is drawn around them. A shape is linked to a node by
//! having the node's id as its element id, so links survive any editor. What both last agreed on
//! (`Synced`) is kept by the caller, which makes each sync a three-way merge: a change on one
//! side only is carried to the other, and when both changed the same thing the source wins.
//!
//! Shapes with text become nodes, arrows between two of them become edges; shapes without
//! text, free text and strokes are decoration and stay in the sketch.
use crate::error::Result;
use crate::excalidraw::{binding, is_shape, label, live, number, scene, text, KEY};
use crate::model::{self, Diagram, EdgeRef, Edit, Head, Node, Rect, Stroke};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The annotation key, on the sketch's own annotation, naming its source diagram.
pub const SOURCE: &str = "mirror";

/// One edge as both sides see it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeKey {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// What the sketch and its source last agreed on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Synced {
    #[serde(default)]
    pub nodes: BTreeMap<String, String>,
    #[serde(default)]
    pub edges: Vec<EdgeKey>,
}
impl Synced {
    pub fn of(diagram: &Diagram) -> Synced {
        let ids: BTreeSet<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
        Synced {
            nodes: diagram
                .nodes
                .iter()
                .map(|n| (n.id.clone(), n.label.clone()))
                .collect(),
            edges: diagram
                .edges
                .iter()
                .filter(|e| ids.contains(e.from.as_str()) && ids.contains(e.to.as_str()))
                .map(|e| EdgeKey {
                    from: e.from.clone(),
                    to: e.to.clone(),
                    label: e.label.clone(),
                })
                .collect(),
        }
    }
}

/// The outcome of a sync: edits for the source (empty unless `two_way`), the new sketch text
/// (unchanged when nothing moved), the source as it will be, and the new agreement.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub edits: Vec<Edit>,
    pub sketch: String,
    pub diagram: Diagram,
    pub synced: Synced,
}

/// A new, empty sketch of `source`.
pub fn new(source: &str, title: Option<&str>) -> String {
    let mut annotation = json!({ "extra": { SOURCE: source } });
    if let Some(title) = title {
        annotation["title"] = json!(title);
    }
    let scene = json!({
        "type": "excalidraw",
        "version": 2,
        "source": "schematlas",
        "elements": [],
        "appState": {},
        "files": {},
        KEY: annotation,
    });
    serde_json::to_string_pretty(&scene).unwrap_or_default() + "\n"
}

/// The source a sketch follows, from its parsed diagram.
pub fn source_of(sketch: &Diagram) -> Option<&str> {
    sketch.annotation.extra.get(SOURCE).map(String::as_str)
}

fn seed(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash % 2_000_000_000
}

/// Model label (`<br>` between lines) to Excalidraw text.
fn shown(label: &str) -> String {
    label
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n")
}

fn text_size(text: &str, font: f64) -> (f64, f64) {
    let lines: Vec<&str> = text.lines().collect();
    let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f64;
    (
        (widest * font * 0.6).max(font),
        lines.len().max(1) as f64 * font * 1.25,
    )
}

/// The size a new shape needs for `label`.
fn shape_size(label: &str) -> (f64, f64) {
    let (w, h) = text_size(&shown(label), 20.0);
    ((w + 48.0).max(160.0), (h + 40.0).max(80.0))
}

fn element(id: &str, kind: &str, rect: Rect) -> Map<String, Value> {
    let s = seed(id);
    let Value::Object(map) = json!({
        "id": id,
        "type": kind,
        "x": rect.x,
        "y": rect.y,
        "width": rect.width,
        "height": rect.height,
        "angle": 0,
        "strokeColor": "#1e1e1e",
        "backgroundColor": "transparent",
        "fillStyle": "solid",
        "strokeWidth": 2,
        "strokeStyle": "solid",
        "roughness": 1,
        "opacity": 100,
        "groupIds": [],
        "frameId": null,
        "roundness": null,
        "seed": s,
        "version": 1,
        "versionNonce": s,
        "isDeleted": false,
        "boundElements": [],
        "updated": 1,
        "link": null,
        "locked": false
    }) else {
        unreachable!()
    };
    map
}

fn text_element(id: &str, container: &str, label: &str, around: Rect, font: f64) -> Value {
    let content = shown(label);
    let (w, h) = text_size(&content, font);
    let mut e = element(
        id,
        "text",
        Rect {
            x: around.x + (around.width - w) / 2.0,
            y: around.y + (around.height - h) / 2.0,
            width: w,
            height: h,
        },
    );
    e.insert("boundElements".into(), Value::Null);
    for (k, v) in [
        ("text", json!(content)),
        ("originalText", json!(content)),
        ("fontSize", json!(font)),
        ("fontFamily", json!(5)),
        ("textAlign", json!("center")),
        ("verticalAlign", json!("middle")),
        ("containerId", json!(container)),
        ("autoResize", json!(true)),
        ("lineHeight", json!(1.25)),
    ] {
        e.insert(k.into(), v);
    }
    Value::Object(e)
}

/// The elements of a sketch, edited in place; every change bumps the element's version so the
/// open editor takes it over its own copy.
struct Scene {
    elements: Vec<Value>,
    changed: bool,
}
impl Scene {
    fn ids(&self) -> BTreeSet<String> {
        self.elements
            .iter()
            .filter_map(|e| e.get("id").and_then(Value::as_str).map(str::to_owned))
            .collect()
    }
    fn unique(&self, base: &str) -> String {
        let ids = self.ids();
        if !ids.contains(base) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !ids.contains(id))
            .unwrap_or_default()
    }
    fn alive(e: &Value) -> bool {
        e.get("isDeleted").and_then(Value::as_bool) != Some(true)
    }
    fn find(&self, id: &str) -> Option<usize> {
        self.elements
            .iter()
            .position(|e| Self::alive(e) && e.get("id").and_then(Value::as_str) == Some(id))
    }
    fn label_of(&self, container: &str) -> Option<usize> {
        self.elements.iter().position(|e| {
            Self::alive(e)
                && e.get("type").and_then(Value::as_str) == Some("text")
                && e.get("containerId").and_then(Value::as_str) == Some(container)
        })
    }
    fn touch(&mut self, i: usize) {
        let e = &mut self.elements[i];
        let version = e.get("version").and_then(Value::as_u64).unwrap_or(1) + 1;
        let id = e
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        e["version"] = json!(version);
        e["versionNonce"] = json!(seed(&format!("{id}{version}")));
        self.changed = true;
    }
    fn delete(&mut self, i: usize) {
        self.elements[i]["isDeleted"] = json!(true);
        self.touch(i);
        let id = self.elements[i]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        while let Some(t) = self.label_of(&id) {
            self.elements[t]["isDeleted"] = json!(true);
            self.touch(t);
        }
        // Arrows attached to a removed shape go with it.
        let attached: Vec<usize> = (0..self.elements.len())
            .filter(|&a| {
                let e = &self.elements[a];
                Self::alive(e)
                    && e.get("type").and_then(Value::as_str) == Some("arrow")
                    && ["startBinding", "endBinding"].iter().any(|k| {
                        e.get(*k)
                            .and_then(|b| b.get("elementId"))
                            .and_then(Value::as_str)
                            == Some(id.as_str())
                    })
            })
            .collect();
        for a in attached {
            self.delete(a);
        }
    }
    fn rect(&self, i: usize) -> Rect {
        let e = self.elements[i].as_object().cloned().unwrap_or_default();
        Rect {
            x: number(&e, "x"),
            y: number(&e, "y"),
            width: number(&e, "width"),
            height: number(&e, "height"),
        }
    }
    fn bind(&mut self, shape: usize, kind: &str, id: &str) {
        let e = &mut self.elements[shape];
        if !e.get("boundElements").is_some_and(Value::is_array) {
            e["boundElements"] = json!([]);
        }
        if let Some(list) = e["boundElements"].as_array_mut() {
            list.push(json!({ "type": kind, "id": id }));
        }
        self.touch(shape);
    }
    /// Set (or remove, for None) the text bound to element `i`.
    fn set_label(&mut self, i: usize, label: Option<&str>, font: f64) {
        let id = self.elements[i]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        match (self.label_of(&id), label) {
            (Some(t), Some(l)) => {
                self.elements[t]["text"] = json!(shown(l));
                self.elements[t]["originalText"] = json!(shown(l));
                self.touch(t);
            }
            (Some(t), None) => {
                self.elements[t]["isDeleted"] = json!(true);
                self.touch(t);
            }
            (None, Some(l)) => {
                let text_id = self.unique(&format!("{id}-label"));
                let around = self.rect(i);
                self.elements
                    .push(text_element(&text_id, &id, l, around, font));
                self.bind(i, "text", &text_id);
            }
            (None, None) => {}
        }
    }
    fn add_node(&mut self, node: &Node, at: Rect) {
        // A shape deleted earlier under this id is replaced.
        self.elements.retain(|e| {
            e.get("id").and_then(Value::as_str) != Some(node.id.as_str())
                && e.get("containerId").and_then(Value::as_str) != Some(node.id.as_str())
        });
        let kind = match node.shape.as_deref() {
            Some("circle" | "doublecircle") => "ellipse",
            Some("diamond") => "diamond",
            _ => "rectangle",
        };
        let mut e = element(&node.id, kind, at);
        if matches!(node.shape.as_deref(), Some("rounded" | "stadium")) {
            e.insert("roundness".into(), json!({ "type": 3 }));
        }
        self.elements.push(Value::Object(e));
        let i = self.elements.len() - 1;
        self.set_label(i, Some(&node.label), 20.0);
        self.changed = true;
    }
    fn add_edge(&mut self, edge: &EdgeKey, head: Head, stroke: Stroke) -> Option<()> {
        let (a, b) = (self.find(&edge.from)?, self.find(&edge.to)?);
        let (ra, rb) = (self.rect(a), self.rect(b));
        let (ca, cb) = (center(ra), center(rb));
        let start = border(ra, cb);
        let end = border(rb, ca);
        let id = self.unique(&format!("{}-{}", edge.from, edge.to));
        let mut e = element(
            &id,
            "arrow",
            Rect {
                x: start.0,
                y: start.1,
                width: (end.0 - start.0).abs(),
                height: (end.1 - start.1).abs(),
            },
        );
        for (k, v) in [
            (
                "points",
                json!([[0.0, 0.0], [end.0 - start.0, end.1 - start.1]]),
            ),
            ("lastCommittedPoint", Value::Null),
            (
                "startBinding",
                json!({ "elementId": edge.from, "focus": 0, "gap": 5 }),
            ),
            (
                "endBinding",
                json!({ "elementId": edge.to, "focus": 0, "gap": 5 }),
            ),
            ("startArrowhead", Value::Null),
            (
                "endArrowhead",
                if head == Head::None {
                    Value::Null
                } else {
                    json!("arrow")
                },
            ),
            ("elbowed", json!(false)),
            ("roundness", json!({ "type": 2 })),
        ] {
            e.insert(k.into(), v);
        }
        match stroke {
            Stroke::Dotted => {
                e.insert("strokeStyle".into(), json!("dashed"));
            }
            Stroke::Thick => {
                e.insert("strokeWidth".into(), json!(4));
            }
            _ => {}
        }
        self.elements.push(Value::Object(e));
        let i = self.elements.len() - 1;
        self.bind(a, "arrow", &id);
        self.bind(b, "arrow", &id);
        if let Some(l) = &edge.label {
            self.set_label(i, Some(l), 16.0);
        }
        self.changed = true;
        Some(())
    }
    /// Where a new node goes: its hint, else beside a connected shape, else below everything.
    fn place(&self, node: &Node, hint: Option<Rect>, neighbors: &[String]) -> Rect {
        let (width, height) = shape_size(&node.label);
        if let Some(h) = hint {
            return Rect {
                width: if h.width > 0.0 { h.width } else { width },
                height: if h.height > 0.0 { h.height } else { height },
                ..h
            };
        }
        let shapes: Vec<Rect> = self
            .elements
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                Self::alive(e)
                    && e.get("type")
                        .and_then(Value::as_str)
                        .is_some_and(|t| t != "text")
            })
            .map(|(i, _)| self.rect(i))
            .collect();
        let free = |r: &Rect| {
            !shapes.iter().any(|s| {
                r.x < s.x + s.width + 20.0
                    && s.x < r.x + r.width + 20.0
                    && r.y < s.y + s.height + 20.0
                    && s.y < r.y + r.height + 20.0
            })
        };
        if let Some(n) = neighbors.iter().find_map(|n| self.find(n)) {
            let r = self.rect(n);
            for step in 0..12 {
                let candidate = Rect {
                    x: r.x + r.width + 120.0,
                    y: r.y + step as f64 * (height + 40.0),
                    width,
                    height,
                };
                if free(&candidate) {
                    return candidate;
                }
            }
        }
        let bottom = shapes.iter().map(|s| s.y + s.height).fold(0.0, f64::max);
        let left = shapes.iter().map(|s| s.x).fold(f64::INFINITY, f64::min);
        let mut x = if left.is_finite() { left } else { 0.0 };
        let y = if shapes.is_empty() {
            0.0
        } else {
            bottom + 80.0
        };
        while !free(&Rect {
            x,
            y,
            width,
            height,
        }) {
            x += width + 40.0;
        }
        Rect {
            x,
            y,
            width,
            height,
        }
    }
}

fn center(r: Rect) -> (f64, f64) {
    (r.x + r.width / 2.0, r.y + r.height / 2.0)
}
/// Where the line from `r`'s centre towards `to` leaves `r`.
fn border(r: Rect, to: (f64, f64)) -> (f64, f64) {
    let (cx, cy) = center(r);
    let (dx, dy) = (to.0 - cx, to.1 - cy);
    if dx == 0.0 && dy == 0.0 {
        return (cx, cy);
    }
    let sx = if dx != 0.0 {
        (r.width / 2.0) / dx.abs()
    } else {
        f64::INFINITY
    };
    let sy = if dy != 0.0 {
        (r.height / 2.0) / dy.abs()
    } else {
        f64::INFINITY
    };
    let s = sx.min(sy);
    (cx + dx * s, cy + dy * s)
}

/// What the sketch shows: linked nodes (id → label) and edges between them, with the element
/// index of each arrow.
fn read(scene: &Map<String, Value>) -> (BTreeMap<String, String>, Vec<(EdgeKey, String)>) {
    let elements = live(scene);
    let mut labels: HashMap<&str, &str> = HashMap::new();
    for e in &elements {
        if let (Some("text"), Some(c), Some(t)) =
            (text(e, "type"), text(e, "containerId"), text(e, "text"))
        {
            labels.insert(c, t);
        }
    }
    let mut nodes = BTreeMap::new();
    for e in &elements {
        let (Some(kind), Some(id)) = (text(e, "type"), text(e, "id")) else {
            continue;
        };
        if !is_shape(kind) || !model::valid_id(id) {
            continue;
        }
        let shown = labels.get(id).map(|t| label(t)).unwrap_or_default();
        if !shown.is_empty() {
            nodes.insert(id.to_owned(), shown);
        }
    }
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
        if nodes.contains_key(from) && nodes.contains_key(to) {
            edges.push((
                EdgeKey {
                    from: from.to_owned(),
                    to: to.to_owned(),
                    label: labels.get(id).map(|t| label(t)).filter(|l| !l.is_empty()),
                },
                id.to_owned(),
            ));
        }
    }
    (nodes, edges)
}

fn shape_of(scene: &Map<String, Value>, id: &str) -> Option<String> {
    let e = live(scene)
        .into_iter()
        .find(|e| text(e, "id") == Some(id))?;
    match text(e, "type")? {
        "ellipse" => Some("circle".into()),
        "diamond" => Some("diamond".into()),
        _ if e.get("roundness").is_some_and(|r| !r.is_null()) => Some("rounded".into()),
        _ => None,
    }
}

fn count<'a>(keys: impl IntoIterator<Item = &'a EdgeKey>) -> BTreeMap<&'a EdgeKey, usize> {
    let mut out = BTreeMap::new();
    for k in keys {
        *out.entry(k).or_insert(0) += 1;
    }
    out
}

/// Bring `sketch` and `diagram` in step from what they last agreed on (`base`). With
/// `two_way`, changes drawn in the sketch become edits of the diagram; without, they stay in
/// the sketch. `hints` place new shapes (a laid-out diagram's positions).
pub fn sync(
    sketch: &str,
    diagram: &Diagram,
    base: &Synced,
    two_way: bool,
    hints: &BTreeMap<String, Rect>,
) -> Result<Outcome> {
    let parsed = scene(sketch)?;
    let (s_nodes, s_edges) = read(&parsed);
    let mut scene = Scene {
        elements: parsed
            .get("elements")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        changed: false,
    };
    let mut working = diagram.clone();
    let mut edits = vec![];
    let mut push = |working: &mut Diagram, edit: Edit| {
        // An edit the diagram refuses (an over-long label…) is left in the sketch only.
        if let Ok(next) = model::apply(working, &edit) {
            *working = next;
            edits.push(edit);
        }
    };

    // Nodes.
    let d_nodes: BTreeMap<&str, &Node> = diagram.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let ids: BTreeSet<&str> = s_nodes
        .keys()
        .map(String::as_str)
        .chain(d_nodes.keys().copied())
        .collect();
    let mut created = vec![];
    for id in ids {
        let in_base = base.nodes.get(id);
        match (s_nodes.get(id), d_nodes.get(id)) {
            (Some(s), Some(d)) => {
                if in_base != Some(&d.label) {
                    // The source changed it (or they never agreed): the source wins.
                    if s != &d.label {
                        if let Some(i) = scene.find(id) {
                            scene.set_label(i, Some(&d.label), 20.0);
                        }
                    }
                } else if s != &d.label && two_way {
                    push(
                        &mut working,
                        Edit::SetLabel {
                            id: id.into(),
                            label: s.clone(),
                        },
                    );
                }
            }
            (Some(s), None) => {
                if in_base.is_some() {
                    if let Some(i) = scene.find(id) {
                        scene.delete(i);
                    }
                } else if two_way {
                    push(
                        &mut working,
                        Edit::AddNode {
                            id: id.into(),
                            label: s.clone(),
                            shape: shape_of(&parsed, id),
                        },
                    );
                }
            }
            (None, Some(d)) => {
                if in_base.is_some() {
                    if two_way {
                        push(&mut working, Edit::RemoveNode { id: id.into() });
                    }
                } else {
                    created.push((*d).clone());
                }
            }
            (None, None) => {}
        }
    }
    for node in &created {
        let neighbors: Vec<String> = diagram
            .edges
            .iter()
            .filter_map(|e| {
                if e.from == node.id {
                    Some(e.to.clone())
                } else if e.to == node.id {
                    Some(e.from.clone())
                } else {
                    None
                }
            })
            .collect();
        let at = scene.place(node, hints.get(&node.id).copied(), &neighbors);
        scene.add_node(node, at);
    }

    // Edges, between nodes both sides still have.
    let alive: BTreeSet<String> = working
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .filter(|id| scene.find(id).is_some())
        .collect();
    let keep = |k: &EdgeKey| alive.contains(&k.from) && alive.contains(&k.to);
    let d_keys: Vec<EdgeKey> = Synced::of(diagram).edges.into_iter().filter(keep).collect();
    let s_keys: Vec<(EdgeKey, String)> = s_edges.into_iter().filter(|(k, _)| keep(k)).collect();
    let b_keys: Vec<EdgeKey> = base.edges.iter().filter(|k| keep(k)).cloned().collect();
    let (dc, sc, bc) = (
        count(&d_keys),
        count(s_keys.iter().map(|(k, _)| k)),
        count(&b_keys),
    );
    let keys: BTreeSet<&EdgeKey> = dc
        .keys()
        .chain(sc.keys())
        .chain(bc.keys())
        .copied()
        .collect();
    let (mut removed, mut added) = (vec![], vec![]);
    for key in keys {
        let (d, s, b) = (
            dc.get(key).copied().unwrap_or(0),
            sc.get(key).copied().unwrap_or(0),
            bc.get(key).copied().unwrap_or(0),
        );
        if d != b {
            // The source changed these edges: the sketch follows.
            if s < d {
                let edge = diagram
                    .edges
                    .iter()
                    .find(|e| e.from == key.from && e.to == key.to && e.label == key.label);
                for _ in s..d {
                    scene.add_edge(
                        key,
                        edge.map_or(Head::Arrow, |e| e.head),
                        edge.map_or(Stroke::Normal, |e| e.stroke),
                    );
                }
            } else {
                let arrows: Vec<&String> = s_keys
                    .iter()
                    .filter(|(k, _)| k == key)
                    .map(|(_, id)| id)
                    .collect();
                for id in arrows.iter().rev().take(s - d) {
                    if let Some(i) = scene.find(id) {
                        scene.delete(i);
                    }
                }
            }
        } else if s != b && two_way {
            for _ in b..s {
                added.push(key.clone());
            }
            for _ in s..b {
                removed.push(key.clone());
            }
        }
    }
    // A relabelled arrow is one edge removed and one added between the same nodes.
    for old in removed {
        let index = working
            .edges
            .iter()
            .rposition(|e| e.from == old.from && e.to == old.to && e.label == old.label);
        let Some(index) = index else { continue };
        let edge = EdgeRef::of(&working, index);
        match added
            .iter()
            .position(|a| a.from == old.from && a.to == old.to)
        {
            Some(a) => {
                let new = added.remove(a);
                push(
                    &mut working,
                    Edit::SetEdgeLabel {
                        edge,
                        label: new.label,
                    },
                );
            }
            None => push(&mut working, Edit::RemoveEdge(edge)),
        }
    }
    for new in added {
        push(
            &mut working,
            Edit::AddEdge {
                from: new.from,
                to: new.to,
                label: new.label,
            },
        );
    }

    let sketch = if scene.changed {
        let mut out = parsed.clone();
        out.insert("elements".into(), Value::Array(scene.elements));
        serde_json::to_string_pretty(&Value::Object(out)).unwrap_or_default() + "\n"
    } else {
        sketch.to_owned()
    };
    let synced = Synced::of(if two_way { &working } else { diagram });
    Ok(Outcome {
        edits: if two_way { edits } else { vec![] },
        sketch,
        diagram: working,
        synced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::excalidraw;
    use crate::model::Edge;

    fn diagram(nodes: &[(&str, &str)], edges: &[(&str, &str, Option<&str>)]) -> Diagram {
        Diagram {
            kind: "flowchart".into(),
            nodes: nodes
                .iter()
                .map(|(id, label)| Node {
                    id: (*id).into(),
                    label: (*label).into(),
                    ..Node::default()
                })
                .collect(),
            edges: edges
                .iter()
                .map(|(from, to, label)| Edge {
                    from: (*from).into(),
                    to: (*to).into(),
                    label: label.map(str::to_owned),
                    ..Edge::default()
                })
                .collect(),
            ..Diagram::default()
        }
    }

    /// The sketch after a first sync of `d`, and what they agree on.
    fn opened(d: &Diagram) -> (String, Synced) {
        let hints = BTreeMap::from([(
            "api".to_owned(),
            Rect {
                x: 0.0,
                y: 0.0,
                width: 160.0,
                height: 80.0,
            },
        )]);
        let out = sync(&new("flow.mmd", None), d, &Synced::default(), true, &hints).unwrap();
        assert!(out.edits.is_empty());
        (out.sketch, out.synced)
    }

    fn edit_scene(sketch: &str, f: impl FnOnce(&mut Vec<Value>)) -> String {
        let mut v: Value = serde_json::from_str(sketch).unwrap();
        f(v["elements"].as_array_mut().unwrap());
        serde_json::to_string_pretty(&v).unwrap()
    }

    #[test]
    fn opening_draws_every_node_and_edge_linked_by_id() {
        let d = diagram(
            &[("api", "API"), ("db", "Orders<br>DB")],
            &[("api", "db", Some("reads"))],
        );
        let (sketch, synced) = opened(&d);
        let read = excalidraw::parse(&sketch).unwrap();
        assert_eq!(source_of(&read), Some("flow.mmd"));
        let ids: Vec<_> = read.nodes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["api", "db"]);
        assert_eq!(read.nodes[1].label, "Orders<br>DB");
        assert_eq!(read.edges.len(), 1);
        assert_eq!(read.edges[0].label.as_deref(), Some("reads"));
        assert_eq!(synced, Synced::of(&d));
        // A second sync has nothing to do.
        let again = sync(&sketch, &d, &synced, true, &BTreeMap::new()).unwrap();
        assert!(again.edits.is_empty());
        assert_eq!(again.sketch, sketch);
    }

    #[test]
    fn drawing_in_the_sketch_edits_the_diagram() {
        let d = diagram(&[("api", "API"), ("db", "DB")], &[("api", "db", None)]);
        let (sketch, synced) = opened(&d);
        // Rename db, add a cache shape with an arrow from api, delete nothing.
        let drawn = edit_scene(&sketch, |els| {
            for e in els.iter_mut() {
                if e["containerId"] == "db" {
                    e["text"] = json!("Postgres");
                }
            }
            els.push(
                json!({"id":"cache","type":"rectangle","x":0,"y":300,"width":160,"height":80}),
            );
            els.push(json!({"id":"cache-t","type":"text","x":0,"y":300,"width":50,"height":25,"text":"Cache","containerId":"cache"}));
            els.push(
                json!({"id":"a2","type":"arrow","x":0,"y":0,"width":1,"height":1,
                "startBinding":{"elementId":"api"},"endBinding":{"elementId":"cache"}}),
            );
            els.push(
                json!({"id":"note","type":"rectangle","x":500,"y":500,"width":50,"height":50}),
            );
        });
        let out = sync(&drawn, &d, &synced, true, &BTreeMap::new()).unwrap();
        assert_eq!(
            out.edits,
            vec![
                Edit::AddNode {
                    id: "cache".into(),
                    label: "Cache".into(),
                    shape: None
                },
                Edit::SetLabel {
                    id: "db".into(),
                    label: "Postgres".into()
                },
                Edit::AddEdge {
                    from: "api".into(),
                    to: "cache".into(),
                    label: None
                },
            ]
        );
        assert_eq!(out.sketch, drawn, "the sketch already shows it");
        assert_eq!(out.synced, Synced::of(&out.diagram));
        // Next time round, nothing to do; the shape without text stays decoration.
        let again = sync(
            &out.sketch,
            &out.diagram,
            &out.synced,
            true,
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(again.edits.is_empty());
        assert_eq!(again.sketch, out.sketch);
    }

    #[test]
    fn deleting_and_relabelling_in_the_sketch() {
        let d = diagram(
            &[("api", "API"), ("db", "DB"), ("queue", "Queue")],
            &[("api", "db", Some("reads")), ("api", "queue", None)],
        );
        let (sketch, synced) = opened(&d);
        let drawn = edit_scene(&sketch, |els| {
            for e in els.iter_mut() {
                if e["id"] == "queue" {
                    e["isDeleted"] = json!(true);
                }
                if e["type"] == "text" && e["containerId"].as_str() == Some("api-db") {
                    e["text"] = json!("writes");
                }
            }
        });
        let out = sync(&drawn, &d, &synced, true, &BTreeMap::new()).unwrap();
        assert!(out.edits.contains(&Edit::RemoveNode { id: "queue".into() }));
        assert!(out.edits.contains(&Edit::SetEdgeLabel {
            edge: EdgeRef {
                from: "api".into(),
                to: "db".into(),
                nth: 0
            },
            label: Some("writes".into())
        }));
        assert_eq!(out.diagram.edges.len(), 1);
    }

    #[test]
    fn changes_to_the_diagram_redraw_the_sketch_and_keep_the_drawing() {
        let d = diagram(&[("api", "API"), ("db", "DB")], &[("api", "db", None)]);
        let (sketch, synced) = opened(&d);
        // Something drawn around the diagram, and a colour on api.
        let drawn = edit_scene(&sketch, |els| {
            for e in els.iter_mut() {
                if e["id"] == "api" {
                    e["backgroundColor"] = json!("#ffc9c9");
                }
            }
            els.push(json!({"id":"doodle","type":"freedraw","x":9,"y":9,"width":5,"height":5}));
        });
        let changed = diagram(
            &[("api", "Gateway"), ("worker", "Worker")],
            &[("api", "worker", Some("enqueues"))],
        );
        let out = sync(&drawn, &changed, &synced, true, &BTreeMap::new()).unwrap();
        assert!(out.edits.is_empty());
        let read = excalidraw::parse(&out.sketch).unwrap();
        let labels: Vec<_> = read
            .nodes
            .iter()
            .map(|n| (n.id.as_str(), n.label.as_str()))
            .collect();
        assert_eq!(labels, [("api", "Gateway"), ("worker", "Worker")]);
        assert_eq!(read.edges.len(), 1);
        assert_eq!(read.edges[0].to, "worker");
        assert!(out.sketch.contains("#ffc9c9") && out.sketch.contains("doodle"));
        // Changed elements carry a newer version, so an open editor takes them.
        let v: Value = serde_json::from_str(&out.sketch).unwrap();
        let label = v["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["containerId"] == "api")
            .unwrap();
        assert!(label["version"].as_u64().unwrap() > 1);
        let again = sync(&out.sketch, &changed, &out.synced, true, &BTreeMap::new()).unwrap();
        assert!(again.edits.is_empty());
        assert_eq!(again.sketch, out.sketch);
    }

    #[test]
    fn one_way_sketches_follow_but_keep_their_own_changes() {
        let d = diagram(&[("a", "A"), ("b", "B")], &[("a", "b", None)]);
        let (sketch, synced) = opened(&d);
        let drawn = edit_scene(&sketch, |els| {
            for e in els.iter_mut() {
                if e["containerId"] == "a" {
                    e["text"] = json!("Mine");
                }
            }
        });
        let out = sync(&drawn, &d, &synced, false, &BTreeMap::new()).unwrap();
        assert!(out.edits.is_empty());
        assert_eq!(out.sketch, drawn);
        assert_eq!(out.diagram, d);
    }
}
