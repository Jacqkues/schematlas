//! draw.io (diagrams.net): pages of mxGraph cells, plain or compressed, with Schematlas
//! annotations stored as the cells' own custom properties (`schematlas-code`, `schematlas-note`…),
//! which draw.io shows and preserves under "Edit Data".
mod xml;

use crate::annotate::Target;
use crate::error::{Error, Result};
use crate::model::{
    self, Annotation, CodeRef, Diagram, Edge, Edit, Group, Head, Node, Rect, Stroke,
};
use base64::Engine as _;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::ops::Range;
use xml::Element;

const PREFIX: &str = "schematlas-";

#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub id: Option<String>,
    pub name: String,
    pub diagram: Diagram,
}

// ---------- Compression (deflate-raw + encodeURIComponent + base64, as draw.io writes) ----------

fn uri_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}
fn uri_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(byte as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}
pub(crate) fn decompress(payload: &str) -> Result<String> {
    let bad = |what: &str| Error::Parse {
        message: format!("compressed draw.io page: {what}"),
        offset: None,
    };
    let raw = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|_| bad("not base64"))?;
    let mut inflated = String::new();
    flate2::read::DeflateDecoder::new(raw.as_slice())
        .read_to_string(&mut inflated)
        .map_err(|_| bad("not deflate data"))?;
    Ok(if inflated.trim_start().starts_with('<') {
        inflated
    } else {
        uri_decode(&inflated)
    })
}
pub(crate) fn compress(xml: &str) -> String {
    let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::best());
    // Writing to a Vec cannot fail.
    let _ = encoder.write_all(uri_encode(xml).as_bytes());
    let raw = encoder.finish().unwrap_or_default();
    base64::engine::general_purpose::STANDARD.encode(raw)
}

// ---------- Locating pages in a file ----------

#[derive(Clone, Debug)]
enum Source {
    /// The page's `<mxGraphModel>` element, in the file itself.
    Inline(Range<usize>),
    /// The compressed text inside `<diagram>`.
    Compressed(Range<usize>),
}
struct RawPage {
    id: Option<String>,
    name: String,
    source: Source,
}

/// The mxfile XML of a file: itself, or the `content` of an exported `.drawio.svg`.
fn mxfile(src: &str) -> Result<(String, bool)> {
    let start = src.trim_start();
    if start.starts_with("<svg") || (start.starts_with("<?xml") && src.contains("<svg")) {
        let elements = xml::index(src)?;
        let content = elements
            .iter()
            .find(|e| e.name == "svg")
            .and_then(|e| e.attr("content"))
            .ok_or_else(|| Error::Parse {
                message: "this SVG has no embedded draw.io diagram".into(),
                offset: None,
            })?;
        return Ok((content.to_owned(), true));
    }
    Ok((src.to_owned(), false))
}

fn pages_of(file: &str) -> Result<(Vec<Element>, Vec<RawPage>)> {
    let elements = xml::index(file)?;
    let mut pages = vec![];
    let Some(root) = elements.first() else {
        return Err(Error::Parse {
            message: "empty draw.io file".into(),
            offset: None,
        });
    };
    match root.name.as_str() {
        "mxGraphModel" => pages.push(RawPage {
            id: None,
            name: "Page-1".into(),
            source: Source::Inline(root.span.clone()),
        }),
        "mxfile" => {
            for &child in &root.children {
                let d = &elements[child];
                if d.name != "diagram" {
                    continue;
                }
                let model = d
                    .children
                    .iter()
                    .map(|&c| &elements[c])
                    .find(|c| c.name == "mxGraphModel");
                let source = match (model, &d.text) {
                    (Some(m), _) => Source::Inline(m.span.clone()),
                    (None, Some(text)) if !file[text.clone()].trim().is_empty() => {
                        let trimmed = file[text.clone()].trim();
                        let offset = file[text.clone()].find(trimmed).unwrap_or(0);
                        Source::Compressed(text.start + offset..text.start + offset + trimmed.len())
                    }
                    _ => continue,
                };
                pages.push(RawPage {
                    id: d.attr("id").map(str::to_owned),
                    name: d.attr("name").unwrap_or("Page").to_owned(),
                    source,
                });
            }
        }
        other => {
            return Err(Error::Parse {
                message: format!("`<{other}>` is not a draw.io document"),
                offset: None,
            });
        }
    }
    if pages.is_empty() {
        return Err(Error::Parse {
            message: "the draw.io file has no pages".into(),
            offset: None,
        });
    }
    Ok((elements, pages))
}

fn page_xml(file: &str, page: &RawPage) -> Result<String> {
    match &page.source {
        Source::Inline(range) => Ok(file[range.clone()].to_owned()),
        Source::Compressed(range) => decompress(&file[range.clone()]),
    }
}

// ---------- Reading cells ----------

/// One mxCell, possibly wrapped in `<UserObject>`/`<object>` that carries its id, label and
/// custom properties.
#[derive(Clone, Debug)]
struct Cell {
    id: String,
    /// The element that represents the cell: the wrapper if any, else the mxCell.
    outer: usize,
    wrapper: Option<usize>,
    cell: usize,
    geometry: Option<usize>,
    label: String,
    style: String,
    vertex: bool,
    edge: bool,
    parent: Option<String>,
    source: Option<String>,
    target: Option<String>,
}

fn style_map(style: &str) -> (Vec<&str>, HashMap<&str, &str>) {
    let mut bare = vec![];
    let mut pairs = HashMap::new();
    for part in style.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        match part.split_once('=') {
            Some((k, v)) => {
                pairs.insert(k, v);
            }
            None => bare.push(part),
        }
    }
    (bare, pairs)
}
fn is_html(style: &str) -> bool {
    style_map(style).1.get("html") == Some(&"1")
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let end = rest.find(';').filter(|e| *e <= 10);
        let decoded = end.and_then(|end| {
            let name = &rest[1..end];
            let c = match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                n if n.starts_with("#x") || n.starts_with("#X") => u32::from_str_radix(&n[2..], 16)
                    .ok()
                    .and_then(char::from_u32),
                n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            }?;
            Some((c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
/// The text of an HTML label: line breaks and blocks become spaces, tags go, entities decode.
fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut tag = String::new();
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
                tag.push(t);
            }
            let name = tag
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or("")
                .to_lowercase();
            if matches!(name.as_str(), "br" | "div" | "p" | "li" | "tr") {
                out.push(' ');
            }
            continue;
        }
        out.push(c);
    }
    decode_entities(&out)
}
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn label_of(raw: &str, style: &str) -> String {
    one_line(&if is_html(style) {
        html_text(raw)
    } else {
        raw.to_owned()
    })
}
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn shape_of(style: &str) -> Option<String> {
    let (bare, pairs) = style_map(style);
    let shape = pairs.get("shape").copied();
    let name = match shape {
        Some("cylinder" | "cylinder3" | "datastore") => "cylinder",
        Some("rhombus") => "diamond",
        Some("hexagon") => "hexagon",
        Some("process") => "subroutine",
        Some("parallelogram") => "lean_right",
        Some("trapezoid") => "trapezoid",
        Some("doubleEllipse") => "doublecircle",
        Some("ellipse") => "circle",
        Some(other) => other,
        None if bare.contains(&"rhombus") => "diamond",
        None if bare.contains(&"ellipse") => "circle",
        None if bare.contains(&"text") => "text",
        None if pairs.get("rounded") == Some(&"1") && pairs.get("arcSize") == Some(&"50") => {
            "stadium"
        }
        None if pairs.get("rounded") == Some(&"1") => "rounded",
        None => return None,
    };
    model::normalize_shape(name)
}
fn style_for(shape: Option<&str>) -> String {
    let base = match shape {
        None => "rounded=0;",
        Some("rounded") => "rounded=1;",
        Some("stadium") => "rounded=1;arcSize=50;",
        Some("cylinder") => "shape=cylinder3;boundedLbl=1;backgroundOutline=1;size=15;",
        Some("diamond") => "rhombus;",
        Some("circle") => "ellipse;aspect=fixed;",
        Some("doublecircle") => "ellipse;shape=doubleEllipse;aspect=fixed;",
        Some("hexagon") => "shape=hexagon;perimeter=hexagonPerimeter2;fixedSize=1;",
        Some("subroutine") => "shape=process;backgroundOutline=1;",
        Some("lean_right") => "shape=parallelogram;perimeter=parallelogramPerimeter;fixedSize=1;",
        Some("trapezoid") => "shape=trapezoid;perimeter=trapezoidPerimeter;fixedSize=1;",
        Some("text") => "text;align=center;",
        Some(other) => return format!("shape={other};whiteSpace=wrap;html=1;"),
    };
    format!("{base}whiteSpace=wrap;html=1;")
}
fn stroke_of(style: &str) -> (Stroke, Head, bool) {
    let (_, pairs) = style_map(style);
    let stroke = if pairs.get("strokeColor") == Some(&"none") {
        Stroke::Invisible
    } else if pairs.get("dashed") == Some(&"1") {
        Stroke::Dotted
    } else if pairs
        .get("strokeWidth")
        .and_then(|w| w.parse::<f64>().ok())
        .is_some_and(|w| w >= 3.0)
    {
        Stroke::Thick
    } else {
        Stroke::Normal
    };
    let head = |arrow: Option<&&str>| match arrow.copied() {
        Some("none") => Head::None,
        Some("oval") => Head::Circle,
        Some("cross") => Head::Cross,
        _ => Head::Arrow,
    };
    let end = head(pairs.get("endArrow"));
    let both = pairs.get("startArrow").is_some_and(|s| *s != "none");
    (stroke, end, both)
}

fn annotation_of(attrs: &[(String, String)]) -> Annotation {
    let mut a = Annotation::default();
    for (key, value) in attrs {
        let Some(key) = key.strip_prefix(PREFIX) else {
            continue;
        };
        match key {
            "code" => {
                a.code = value
                    .split(['\n', ';'])
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .map(CodeRef::parse)
                    .filter(|c| c.validate().is_ok())
                    .collect()
            }
            "note" => a.note = Some(value.clone()),
            "title" => a.title = Some(value.clone()),
            "kind" => a.kind = Some(value.clone()),
            "tags" => {
                a.tags = value
                    .split(',')
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
            other => {
                a.extra.insert(other.to_owned(), value.clone());
            }
        }
    }
    a
}
fn annotation_attrs(a: &Annotation) -> Vec<(String, String)> {
    let mut out = vec![];
    if let Some(title) = &a.title {
        out.push((format!("{PREFIX}title"), title.clone()));
    }
    if let Some(kind) = &a.kind {
        out.push((format!("{PREFIX}kind"), kind.clone()));
    }
    if !a.code.is_empty() {
        out.push((
            format!("{PREFIX}code"),
            a.code
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
        ));
    }
    if let Some(note) = &a.note {
        out.push((format!("{PREFIX}note"), note.clone()));
    }
    if !a.tags.is_empty() {
        out.push((format!("{PREFIX}tags"), a.tags.join(",")));
    }
    for (key, value) in &a.extra {
        out.push((format!("{PREFIX}{key}"), value.clone()));
    }
    out
}

struct Graph {
    elements: Vec<Element>,
    cells: Vec<Cell>,
    root_element: Option<usize>,
}

fn read_cells(page: &str) -> Result<Graph> {
    let elements = xml::index(page)?;
    let root_element = elements.iter().position(|e| e.name == "root");
    let mut cells = vec![];
    if let Some(root) = root_element {
        for &child in &elements[root].children {
            let e = &elements[child];
            let (wrapper, cell) = match e.name.as_str() {
                "mxCell" => (None, child),
                "UserObject" | "object" => match e
                    .children
                    .iter()
                    .copied()
                    .find(|&c| elements[c].name == "mxCell")
                {
                    Some(c) => (Some(child), c),
                    None => continue,
                },
                _ => continue,
            };
            let c = &elements[cell];
            let id_holder = wrapper.map_or(c, |w| &elements[w]);
            let Some(id) = id_holder.attr("id") else {
                continue;
            };
            let style = c.attr("style").unwrap_or("").to_owned();
            let raw = match wrapper {
                Some(w) => elements[w].attr("label").unwrap_or(""),
                None => c.attr("value").unwrap_or(""),
            };
            cells.push(Cell {
                id: id.to_owned(),
                outer: child,
                wrapper,
                cell,
                geometry: c
                    .children
                    .iter()
                    .copied()
                    .find(|&g| elements[g].name == "mxGeometry"),
                label: label_of(raw, &style),
                vertex: c.attr("vertex") == Some("1"),
                edge: c.attr("edge") == Some("1"),
                parent: c.attr("parent").map(str::to_owned),
                source: c.attr("source").map(str::to_owned),
                target: c.attr("target").map(str::to_owned),
                style,
            });
        }
    }
    Ok(Graph {
        elements,
        cells,
        root_element,
    })
}

impl Graph {
    fn cell(&self, id: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.id == id)
    }
    fn is_edge(&self, id: &str) -> bool {
        self.cell(id).is_some_and(|c| c.edge)
    }
    /// Vertices that hold other vertices, or are styled as containers.
    fn is_container(&self, c: &Cell) -> bool {
        let (bare, pairs) = style_map(&c.style);
        c.vertex
            && (bare.contains(&"swimlane")
                || bare.contains(&"group")
                || pairs.get("container") == Some(&"1")
                || self.cells.iter().any(|o| {
                    o.vertex && o.parent.as_deref() == Some(&c.id) && !self.is_edge(&c.id)
                }))
    }
    fn geometry(&self, c: &Cell) -> Rect {
        let Some(g) = c.geometry else {
            return Rect::default();
        };
        let num = |k: &str| {
            self.elements[g]
                .attr(k)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        Rect {
            x: num("x"),
            y: num("y"),
            width: num("width"),
            height: num("height"),
        }
    }
    /// Position of a cell's own coordinate origin: the absolute position of its container.
    fn origin(&self, c: &Cell) -> (f64, f64) {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut parent = c.parent.clone();
        let mut guard = 0;
        while let Some(p) = parent.and_then(|p| self.cell(&p)) {
            if !p.vertex || guard > 64 {
                break;
            }
            let g = self.geometry(p);
            x += g.x;
            y += g.y;
            parent = p.parent.clone();
            guard += 1;
        }
        (x, y)
    }
    fn absolute(&self, c: &Cell) -> Rect {
        let (ox, oy) = self.origin(c);
        let g = self.geometry(c);
        Rect {
            x: g.x + ox,
            y: g.y + oy,
            ..g
        }
    }
    fn default_layer(&self) -> String {
        let root = self
            .cells
            .iter()
            .find(|c| c.parent.is_none())
            .map(|c| c.id.clone());
        self.cells
            .iter()
            .find(|c| c.parent.is_some() && c.parent == root && !c.vertex && !c.edge)
            .map_or_else(|| "1".to_owned(), |c| c.id.clone())
    }
    fn edge_labels(&self, edge: &str) -> Vec<&Cell> {
        self.cells
            .iter()
            .filter(|c| c.vertex && c.parent.as_deref() == Some(edge))
            .collect()
    }

    fn diagram(&self) -> Diagram {
        let containers: Vec<&Cell> = self
            .cells
            .iter()
            .filter(|c| !self.parent_is_edge(c) && self.is_container(c))
            .collect();
        let group_ids: Vec<&str> = containers.iter().map(|c| c.id.as_str()).collect();
        let mut d = Diagram {
            kind: model::DRAWIO.into(),
            ..Diagram::default()
        };
        for c in &self.cells {
            if c.parent.is_none() {
                d.annotation = c
                    .wrapper
                    .map(|w| annotation_of(&self.elements[w].attrs))
                    .unwrap_or_default();
            }
            if !c.vertex || self.parent_is_edge(c) || group_ids.contains(&c.id.as_str()) {
                continue;
            }
            d.layout.insert(c.id.clone(), self.absolute(c));
            let wrapper = c.wrapper.map(|w| &self.elements[w]);
            d.nodes.push(Node {
                id: c.id.clone(),
                label: c.label.clone(),
                shape: shape_of(&c.style),
                link: wrapper.and_then(|w| w.attr("link")).map(str::to_owned),
                classes: vec![],
                details: vec![],
                annotation: wrapper.map(|w| annotation_of(&w.attrs)).unwrap_or_default(),
            });
        }
        for g in &containers {
            d.layout.insert(g.id.clone(), self.absolute(g));
            d.groups.push(Group {
                id: g.id.clone(),
                title: if g.label.is_empty() {
                    g.id.clone()
                } else {
                    g.label.clone()
                },
                nodes: d
                    .nodes
                    .iter()
                    .filter(|n| {
                        self.cell(&n.id).and_then(|c| c.parent.as_deref()) == Some(g.id.as_str())
                    })
                    .map(|n| n.id.clone())
                    .collect(),
                parent: g.parent.clone().filter(|p| group_ids.contains(&p.as_str())),
                direction: None,
            });
        }
        let ends: Vec<&str> = d
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .chain(group_ids.iter().copied())
            .collect();
        for c in &self.cells {
            let (Some(from), Some(to)) = (&c.source, &c.target) else {
                continue;
            };
            if !c.edge || !ends.contains(&from.as_str()) || !ends.contains(&to.as_str()) {
                continue;
            }
            let label = if c.label.is_empty() {
                self.edge_labels(&c.id)
                    .iter()
                    .map(|l| l.label.clone())
                    .find(|l| !l.is_empty())
            } else {
                Some(c.label.clone())
            };
            let (stroke, head, both_ends) = stroke_of(&c.style);
            d.edges.push(Edge {
                id: Some(c.id.clone()),
                from: from.clone(),
                to: to.clone(),
                label,
                stroke,
                head,
                both_ends,
                length: 1,
                tail_label: None,
                head_label: None,
                annotation: c
                    .wrapper
                    .map(|w| annotation_of(&self.elements[w].attrs))
                    .unwrap_or_default(),
            });
        }
        d
    }
    fn parent_is_edge(&self, c: &Cell) -> bool {
        c.parent.as_deref().is_some_and(|p| self.is_edge(p))
    }
}

/// Every page of a `.drawio` file (or of an exported `.drawio.svg`).
pub fn parse(src: &str) -> Result<Vec<Page>> {
    let (file, _) = mxfile(src)?;
    let (_, pages) = pages_of(&file)?;
    pages
        .iter()
        .map(|p| {
            let xml = page_xml(&file, p)?;
            Ok(Page {
                id: p.id.clone(),
                name: p.name.clone(),
                diagram: read_cells(&xml)?.diagram(),
            })
        })
        .collect()
}

// ---------- Writing ----------

struct Patches(Vec<(Range<usize>, String)>);
impl Patches {
    fn apply(mut self, text: &str) -> Result<String> {
        self.0
            .sort_by(|a, b| a.0.start.cmp(&b.0.start).then(a.0.end.cmp(&b.0.end)));
        let mut merged: Vec<(Range<usize>, String)> = vec![];
        for (range, value) in self.0 {
            if let Some((last, last_value)) = merged.last_mut() {
                if range.start < last.end {
                    if value.is_empty() && last_value.is_empty() {
                        last.end = last.end.max(range.end);
                        continue;
                    }
                    return Err(Error::InvalidEdit("overlapping changes".into()));
                }
            }
            merged.push((range, value));
        }
        let mut out = text.to_owned();
        for (range, value) in merged.into_iter().rev() {
            out.replace_range(range, &value);
        }
        Ok(out)
    }
}

/// The element's span widened to its whole line when nothing else is on that line.
fn removal_span(text: &str, span: &Range<usize>) -> Range<usize> {
    let line_start = text[..span.start].rfind('\n').map_or(0, |p| p + 1);
    let line_end = text[span.end..]
        .find('\n')
        .map_or(text.len(), |p| span.end + p + 1);
    let alone = text[line_start..span.start].trim().is_empty()
        && text[span.end..line_end].trim().is_empty();
    if alone {
        line_start..line_end
    } else {
        span.clone()
    }
}
fn indent_before(text: &str, offset: usize) -> String {
    let line_start = text[..offset].rfind('\n').map_or(0, |p| p + 1);
    text[line_start..offset]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect()
}
fn number(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

struct Writer<'a> {
    text: &'a str,
    graph: Graph,
    before: Diagram,
    patches: Patches,
}
impl Writer<'_> {
    fn element(&self, i: usize) -> &Element {
        &self.graph.elements[i]
    }
    fn set_attrs(&mut self, element: usize, changes: &[(&str, Option<String>)]) {
        let e = self.element(element).clone();
        let mut attrs = e.attrs.clone();
        for (key, value) in changes {
            match (attrs.iter().position(|(k, _)| k == key), value) {
                (Some(i), Some(v)) => attrs[i].1 = v.clone(),
                (Some(i), None) => {
                    attrs.remove(i);
                }
                (None, Some(v)) => attrs.push(((*key).to_owned(), v.clone())),
                (None, None) => {}
            }
        }
        self.patches
            .0
            .push((e.tag.clone(), xml::start_tag(&e.name, &attrs, e.empty)));
    }
    fn remove_element(&mut self, element: usize) {
        let span = removal_span(self.text, &self.element(element).span);
        self.patches.0.push((span, String::new()));
    }
    fn insert_cell(&mut self, markup: String) -> Result<()> {
        let root = self
            .graph
            .root_element
            .ok_or_else(|| Error::InvalidEdit("the page has no <root>".into()))?;
        let span = self.element(root).span.clone();
        let close = span.start
            + self.text[span.clone()]
                .rfind("</root>")
                .ok_or_else(|| Error::InvalidEdit("the page has no </root>".into()))?;
        let last_child = self
            .element(root)
            .children
            .last()
            .map(|&c| self.element(c).span.start);
        let indent = last_child.map_or_else(|| "  ".to_owned(), |s| indent_before(self.text, s));
        let line_start = self.text[..close].rfind('\n').map_or(0, |p| p + 1);
        let at = if self.text[line_start..close].trim().is_empty() {
            line_start
        } else {
            close
        };
        let lead = if at == close { "\n" } else { "" };
        self.patches
            .0
            .push((at..at, format!("{lead}{indent}{markup}\n")));
        Ok(())
    }
    fn label_change(&mut self, cell: &Cell, label: Option<&str>) {
        let text = label.map(|l| {
            if is_html(&cell.style) {
                html_escape(l)
            } else {
                l.to_owned()
            }
        });
        match cell.wrapper {
            Some(w) => self.set_attrs(w, &[("label", Some(text.unwrap_or_default()))]),
            None => self.set_attrs(cell.cell, &[("value", Some(text.unwrap_or_default()))]),
        }
    }
    fn cell(&self, id: &str) -> Result<Cell> {
        self.graph
            .cell(id)
            .cloned()
            .ok_or_else(|| Error::InvalidEdit(format!("no cell `{id}`")))
    }
    fn edge_cell(&self, edge: &model::EdgeRef) -> Result<Cell> {
        let index = edge
            .index(&self.before)
            .ok_or_else(|| Error::InvalidEdit("no such edge".into()))?;
        let id = self.before.edges[index].id.clone().unwrap_or_default();
        self.cell(&id)
    }
    fn remove_edge_cell(&mut self, cell: &Cell) {
        self.remove_element(cell.outer);
        let labels: Vec<usize> = self
            .graph
            .edge_labels(&cell.id)
            .iter()
            .map(|l| l.outer)
            .collect();
        for l in labels {
            self.remove_element(l);
        }
    }
    fn annotate(&mut self, cell: &Cell, annotation: &Annotation) {
        let fresh = annotation_attrs(annotation);
        match cell.wrapper {
            Some(w) => {
                let e = self.element(w).clone();
                let mut attrs: Vec<(String, String)> = e
                    .attrs
                    .iter()
                    .filter(|(k, _)| !k.starts_with(PREFIX))
                    .cloned()
                    .collect();
                // Keep `id` last, where draw.io writes it.
                let id = attrs
                    .iter()
                    .position(|(k, _)| k == "id")
                    .map(|i| attrs.remove(i));
                attrs.extend(fresh);
                attrs.extend(id);
                self.patches
                    .0
                    .push((e.tag.clone(), xml::start_tag(&e.name, &attrs, e.empty)));
            }
            None if fresh.is_empty() => {}
            None => {
                // Wrap the cell in a UserObject, which is how draw.io stores custom properties.
                let c = self.element(cell.cell).clone();
                let label = c.attr("value").unwrap_or("").to_owned();
                let inner: Vec<(String, String)> = c
                    .attrs
                    .iter()
                    .filter(|(k, _)| k != "id" && k != "value")
                    .cloned()
                    .collect();
                let mut outer = vec![("label".to_owned(), label)];
                outer.extend(fresh);
                outer.push(("id".to_owned(), cell.id.clone()));
                let indent = indent_before(self.text, c.span.start);
                let inner_tag = xml::start_tag("mxCell", &inner, c.empty);
                let rest = &self.text[c.tag.end..c.span.end];
                let body: String = format!("{inner_tag}{rest}")
                    .split_inclusive('\n')
                    .enumerate()
                    .map(|(i, line)| {
                        if i == 0 {
                            line.to_owned()
                        } else {
                            format!("  {line}")
                        }
                    })
                    .collect();
                let markup = format!(
                    "{}\n{indent}  {body}\n{indent}</UserObject>",
                    xml::start_tag("UserObject", &outer, false)
                );
                self.patches.0.push((c.span.clone(), markup));
            }
        }
    }

    fn run(&mut self, edit: &Edit, expected: &Diagram) -> Result<()> {
        match edit {
            Edit::SetLabel { id, label } => {
                let cell = self.cell(id)?;
                self.label_change(&cell, Some(label.trim()));
            }
            Edit::AddNode { id, label, shape } => {
                let spot = expected
                    .layout
                    .get(id)
                    .copied()
                    .unwrap_or_else(|| self.before.free_spot());
                let shape = shape.as_deref().and_then(model::normalize_shape);
                let markup = format!(
                    "{}<mxGeometry x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" as=\"geometry\" /></mxCell>",
                    xml::start_tag(
                        "mxCell",
                        &[
                            ("id".into(), id.clone()),
                            ("value".into(), html_escape(label.trim())),
                            ("style".into(), style_for(shape.as_deref())),
                            ("vertex".into(), "1".into()),
                            ("parent".into(), self.graph.default_layer()),
                        ],
                        false
                    ),
                    number(spot.x),
                    number(spot.y),
                    number(spot.width),
                    number(spot.height)
                );
                self.insert_cell(markup)?;
            }
            Edit::AddEdge { from, to, label } => {
                let id = self.before.new_edge_id(from, to);
                let label = label.as_deref().map(str::trim).unwrap_or("");
                let markup = format!(
                    "{}<mxGeometry relative=\"1\" as=\"geometry\" /></mxCell>",
                    xml::start_tag(
                        "mxCell",
                        &[
                            ("id".into(), id),
                            ("value".into(), html_escape(label)),
                            ("style".into(), "edgeStyle=orthogonalEdgeStyle;rounded=0;orthogonalLoop=1;jettySize=auto;html=1;".into()),
                            ("edge".into(), "1".into()),
                            ("parent".into(), self.graph.default_layer()),
                            ("source".into(), from.clone()),
                            ("target".into(), to.clone()),
                        ],
                        false
                    )
                );
                self.insert_cell(markup)?;
            }
            Edit::RemoveNode { id } => {
                let cell = self.cell(id)?;
                self.remove_element(cell.outer);
                let edges: Vec<Cell> = self
                    .graph
                    .cells
                    .iter()
                    .filter(|c| {
                        c.edge
                            && (c.source.as_deref() == Some(id) || c.target.as_deref() == Some(id))
                    })
                    .cloned()
                    .collect();
                for e in edges {
                    self.remove_edge_cell(&e);
                }
            }
            Edit::RemoveEdge(edge) => {
                let cell = self.edge_cell(edge)?;
                self.remove_edge_cell(&cell);
            }
            Edit::SetEdgeLabel { edge, label } => {
                let cell = self.edge_cell(edge)?;
                let label = label.as_deref().map(str::trim).filter(|l| !l.is_empty());
                let children: Vec<Cell> = self
                    .graph
                    .edge_labels(&cell.id)
                    .into_iter()
                    .cloned()
                    .collect();
                match label {
                    Some(text) if cell.label.is_empty() && !children.is_empty() => {
                        let first = &children[0];
                        self.label_change(first, Some(text));
                    }
                    Some(text) => self.label_change(&cell, Some(text)),
                    None => {
                        if !cell.label.is_empty() {
                            self.label_change(&cell, None);
                        }
                        for child in children {
                            self.remove_element(child.outer);
                        }
                    }
                }
            }
            Edit::Annotate { target, annotation } => {
                let cell = match target {
                    Target::Diagram => self
                        .graph
                        .cells
                        .iter()
                        .find(|c| c.parent.is_none())
                        .cloned()
                        .ok_or_else(|| Error::InvalidEdit("the page has no root cell".into()))?,
                    Target::Node(id) => self.cell(id)?,
                    Target::Edge(edge) => self.edge_cell(edge)?,
                };
                self.annotate(&cell, annotation);
            }
            Edit::Move { id, .. } => {
                let cell = self.cell(id)?;
                let target = expected.layout.get(id).copied().unwrap_or_default();
                let (ox, oy) = self.graph.origin(&cell);
                let geometry = cell
                    .geometry
                    .ok_or_else(|| Error::InvalidEdit(format!("`{id}` has no geometry")))?;
                self.set_attrs(
                    geometry,
                    &[
                        ("x", Some(number(target.x - ox))),
                        ("y", Some(number(target.y - oy))),
                    ],
                );
            }
        }
        Ok(())
    }
}

fn edit_page(xml: &str, edit: &Edit) -> Result<String> {
    let graph = read_cells(xml)?;
    let before = graph.diagram();
    let expected = model::apply(&before, edit)?;
    let mut writer = Writer {
        text: xml,
        graph,
        before,
        patches: Patches(vec![]),
    };
    writer.run(edit, &expected)?;
    let out = std::mem::replace(&mut writer.patches, Patches(vec![])).apply(xml)?;
    let after = read_cells(&out)
        .map_err(|e| Error::InvalidEdit(format!("this edit would break the page ({e})")))?
        .diagram();
    if !after.same_as(&expected) {
        return Err(Error::InvalidEdit(
            "this edit cannot be applied to the page safely".into(),
        ));
    }
    Ok(out)
}

/// Apply an edit to one page; every other byte of the file is kept, and a compressed page is
/// written back compressed.
pub fn apply(src: &str, page: usize, edit: &Edit) -> Result<String> {
    let (file, svg) = mxfile(src)?;
    if svg {
        return Err(Error::InvalidEdit(
            "exported .drawio.svg images are read-only here; edit the .drawio file they came from"
                .into(),
        ));
    }
    let (_, pages) = pages_of(&file)?;
    let raw = pages
        .get(page)
        .ok_or_else(|| Error::InvalidEdit(format!("no page #{page}")))?;
    let xml = page_xml(&file, raw)?;
    let edited = edit_page(&xml, edit)?;
    let (range, text) = match &raw.source {
        Source::Inline(range) => (range.clone(), edited),
        Source::Compressed(range) => (range.clone(), compress(&edited)),
    };
    let mut out = file;
    out.replace_range(range, &text);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compression_round_trips_unicode_and_markup() {
        let xml = "<mxGraphModel><root><mxCell id=\"0\" value=\"Ünïcode &amp; 🚀 100%\"/></root></mxGraphModel>";
        assert_eq!(decompress(&compress(xml)).unwrap(), xml);
        assert_eq!(uri_decode(&uri_encode("a b/é%")), "a b/é%");
    }

    #[test]
    fn html_labels_become_text() {
        assert_eq!(label_of("Orders<br>API", "html=1;"), "Orders API");
        assert_eq!(
            label_of("<div><b>Bold</b>&nbsp;&amp; more</div>", "html=1"),
            "Bold & more"
        );
        assert_eq!(label_of("a <b> literal", "rounded=1"), "a <b> literal");
    }
}
