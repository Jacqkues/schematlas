//! Edits applied to Mermaid text as small patches.
//!
//! Labels are replaced inside their original delimiters, new nodes and edges are appended as new
//! lines, and only a statement that must change shape (removing one edge of a chain, say) is
//! rewritten — and then only that statement. Every result is re-read and compared with the
//! model's own view of the edit before it is returned, so a writer bug can fail an edit but never
//! corrupt a file.
use super::source::{self, Facts, Kind, Layout, Mention, Statement};
use crate::annotate::{self, Target};
use crate::error::{Error, Result};
use crate::model::{self, Diagram, EdgeRef, Edit, Head, Stroke};
use serde_json::Value;
use std::collections::HashSet;
use std::ops::Range;

const HEADER: &str = "flowchart LR\n";

struct Ctx<'a> {
    src: &'a str,
    layout: Layout,
    facts: Facts,
    diagram: &'a Diagram,
    patches: Vec<(Range<usize>, String)>,
}

/// The text of `src` with `edit` applied.
pub fn apply(src: &str, edit: &Edit) -> Result<String> {
    let family = super::family(src)?;
    if family != super::Family::Flowchart && !matches!(edit, Edit::Annotate { .. }) {
        return Err(Error::Unsupported(family.name().to_owned()));
    }
    let before = super::parse(src)?.diagram;
    let expected = model::apply(&before, edit)?;
    let mut ctx = Ctx {
        src,
        layout: source::layout(src),
        facts: source::facts(src),
        diagram: &before,
        patches: vec![],
    };
    match edit {
        Edit::AddNode { id, label, shape } => {
            let text = node_text(
                id,
                label.trim(),
                shape.as_deref().and_then(model::normalize_shape).as_deref(),
            );
            ctx.append_line(&text);
        }
        Edit::AddEdge { from, to, label } => {
            let label = label.as_deref().map(str::trim).filter(|l| !l.is_empty());
            ctx.append_line(&format!(
                "{from} {} {to}",
                arrow_with_label(Stroke::Normal, Head::Arrow, false, 1, None, label)
            ));
        }
        Edit::SetLabel { id, label } => ctx.set_label(id, label.trim())?,
        Edit::RemoveNode { id } => ctx.remove_node(id)?,
        Edit::RemoveEdge(edge) => ctx.remove_edge(edge)?,
        Edit::SetEdgeLabel { edge, label } => {
            let label = label.as_deref().map(str::trim).filter(|l| !l.is_empty());
            ctx.set_edge_label(edge, label)?
        }
        Edit::Annotate { target, annotation } => ctx.annotate(target, annotation),
        // Already refused by `model::apply`: Mermaid keeps no positions.
        Edit::Move { .. } => {
            return Err(Error::InvalidEdit(
                "Mermaid diagrams do not store positions".into(),
            ))
        }
    }
    let mut out = ctx.finish()?;
    // Lines this writer adds use `\n`; a CRLF file keeps CRLF throughout.
    if src.contains("\r\n") {
        out = out.replace("\r\n", "\n").replace('\n', "\r\n");
    }
    let after = super::parse(&out)
        .map_err(|e| Error::InvalidEdit(format!("this edit would break the diagram ({e})")))?
        .diagram;
    if !after.same_as(&expected) {
        return Err(Error::InvalidEdit(
            "this edit cannot be applied to the file without changing other parts of it".into(),
        ));
    }
    Ok(out)
}

// ---------- Mermaid text building ----------

/// Whether a label must be written as a quoted string.
fn needs_quotes(label: &str) -> bool {
    !label
        .chars()
        .all(|c| c.is_alphanumeric() || " _-.,'!?+*/:".contains(c))
}
fn entities(label: &str) -> String {
    label.replace('"', "#quot;")
}
/// A label for a position that is not already inside quotes.
fn bare_or_quoted(label: &str) -> String {
    if needs_quotes(label) {
        format!("\"{}\"", entities(label))
    } else {
        label.to_owned()
    }
}

fn delimiters(shape: Option<&str>) -> Option<(&'static str, &'static str)> {
    Some(match shape {
        None => ("[", "]"),
        Some("rounded") => ("(", ")"),
        Some("stadium") => ("([", "])"),
        Some("subroutine") => ("[[", "]]"),
        Some("cylinder") => ("[(", ")]"),
        Some("circle") => ("((", "))"),
        Some("doublecircle") => ("(((", ")))"),
        Some("asymmetric") => (">", "]"),
        Some("diamond") => ("{", "}"),
        Some("hexagon") => ("{{", "}}"),
        Some("lean_right") => ("[/", "/]"),
        Some("lean_left") => ("[\\", "\\]"),
        Some("trapezoid") => ("[/", "\\]"),
        Some("inv_trapezoid") => ("[\\", "/]"),
        Some(_) => return None,
    })
}
fn node_text(id: &str, label: &str, shape: Option<&str>) -> String {
    match delimiters(shape) {
        Some((open, close)) => format!("{id}{open}{}{close}", bare_or_quoted(label)),
        None => format!(
            "{id}@{{ shape: {}, label: \"{}\" }}",
            shape.unwrap_or("rect"),
            entities(label)
        ),
    }
}

fn arrow(stroke: Stroke, head: Head, both: bool, length: u8) -> String {
    let length = length.max(1) as usize;
    // Flowcharts have only these heads; UML heads come from class diagrams, which are not edited.
    let tip = match head {
        Head::Arrow | Head::Triangle | Head::Diamond | Head::FilledDiamond => ">",
        Head::Circle => "o",
        Head::Cross => "x",
        Head::None => "",
    };
    let tail = if both {
        match head {
            Head::Arrow | Head::Triangle | Head::Diamond | Head::FilledDiamond => "<",
            Head::Circle => "o",
            Head::Cross => "x",
            Head::None => "",
        }
    } else {
        ""
    };
    let open = head == Head::None;
    let body = match stroke {
        Stroke::Invisible => return "~".repeat(length + 2),
        Stroke::Normal => "-".repeat(if open { length + 2 } else { length + 1 }),
        Stroke::Thick => "=".repeat(if open { length + 2 } else { length + 1 }),
        Stroke::Dotted => format!("-{}-", ".".repeat(length)),
    };
    format!("{tail}{body}{tip}")
}
fn arrow_with_label(
    stroke: Stroke,
    head: Head,
    both: bool,
    length: u8,
    id: Option<&str>,
    label: Option<&str>,
) -> String {
    let mut text = String::new();
    if let Some(id) = id {
        text.push_str(id);
        text.push('@');
    }
    text.push_str(&arrow(stroke, head, both, length));
    if let Some(label) = label {
        text.push('|');
        text.push_str(&bare_or_quoted(label));
        text.push('|');
    }
    text
}

// ---------- One statement, read on its own ----------

struct MiniEdge {
    from: String,
    to: String,
    label: Option<String>,
    stroke: Stroke,
    head: Head,
    both: bool,
    length: u8,
    id: Option<String>,
}
struct Mini {
    edges: Vec<MiniEdge>,
    /// Mentioned ids in first-appearance order, with the fullest definition text seen for each.
    mentions: Vec<(String, String)>,
    /// Edge label payloads, relative to the statement text.
    labels: Vec<(Range<usize>, Range<usize>)>,
}

fn mini(text: &str) -> Result<Mini> {
    let wrapped = format!("{HEADER}{text}\n");
    let model = super::merman_model(&wrapped)
        .map_err(|e| Error::InvalidEdit(format!("cannot edit the statement `{text}` ({e})")))?;
    let empty = vec![];
    let edges = model["edges"]
        .as_array()
        .unwrap_or(&empty)
        .iter()
        .map(|e: &Value| {
            let (stroke, head, both) = super::stroke_and_head(e);
            MiniEdge {
                from: e["from"].as_str().unwrap_or_default().to_owned(),
                to: e["to"].as_str().unwrap_or_default().to_owned(),
                label: super::text(&e["label"]),
                stroke,
                head,
                both,
                length: e["length"].as_u64().unwrap_or(1).clamp(1, 20) as u8,
                id: e["isUserDefinedId"]
                    .as_bool()
                    .unwrap_or(false)
                    .then(|| super::text(&e["id"]))
                    .flatten(),
            }
        })
        .collect();
    let facts = source::facts(&wrapped);
    let offset = HEADER.len();
    let mut mentions: Vec<(String, String)> = vec![];
    for m in &facts.mentions {
        if m.span.start < offset {
            continue;
        }
        let def = wrapped[m.span.start..m.def_end].to_owned();
        match mentions.iter_mut().find(|(id, _)| *id == m.id) {
            Some((_, existing)) if def.len() > existing.len() => *existing = def,
            Some(_) => {}
            None => mentions.push((m.id.clone(), def)),
        }
    }
    let shift = |r: &Range<usize>| r.start - offset..r.end - offset;
    let labels = facts
        .edge_labels
        .iter()
        .filter(|(whole, _)| whole.start >= offset)
        .map(|(whole, inner)| (shift(whole), shift(inner)))
        .collect();
    Ok(Mini {
        edges,
        mentions,
        labels,
    })
}

/// Rewrite one statement as lines that keep its surviving nodes (with their definitions) in the
/// same first-appearance order, and its surviving edges in the same order.
fn regenerate(
    mini: &Mini,
    drop_nodes: &HashSet<String>,
    keep_edge: impl Fn(usize) -> bool,
    relabel: Option<(usize, Option<&str>)>,
) -> Vec<String> {
    let order: Vec<&str> = mini
        .mentions
        .iter()
        .map(|(id, _)| id.as_str())
        .filter(|id| !drop_nodes.contains(*id))
        .collect();
    let def = |id: &str| {
        mini.mentions
            .iter()
            .find(|(m, _)| m == id)
            .map_or(id.to_owned(), |(_, d)| d.clone())
    };
    let mut lines = vec![];
    let mut seen: HashSet<&str> = HashSet::new();
    let mut ptr = 0;
    let advance = |seen: &HashSet<&str>, ptr: &mut usize| {
        while *ptr < order.len() && seen.contains(order[*ptr]) {
            *ptr += 1;
        }
    };
    for (i, edge) in mini.edges.iter().enumerate() {
        if !keep_edge(i) || drop_nodes.contains(&edge.from) || drop_nodes.contains(&edge.to) {
            continue;
        }
        let mut need: Vec<&str> = vec![];
        for id in [edge.from.as_str(), edge.to.as_str()] {
            if !seen.contains(id) && !need.contains(&id) {
                need.push(id);
            }
        }
        if let Some(first) = need.first().copied() {
            while ptr < order.len() && order[ptr] != first {
                lines.push(def(order[ptr]));
                seen.insert(order[ptr]);
                ptr += 1;
                advance(&seen, &mut ptr);
            }
            if need.len() == 2 && order.get(ptr + 1).copied() != Some(need[1]) {
                // The two new endpoints are not adjacent in the original order: introduce every
                // node up to the second one first, then write the edge with plain ids.
                if let Some(stop) = order.iter().position(|o| *o == need[1]) {
                    while ptr <= stop && ptr < order.len() {
                        if !seen.contains(order[ptr]) {
                            lines.push(def(order[ptr]));
                            seen.insert(order[ptr]);
                        }
                        ptr += 1;
                    }
                }
            }
        }
        let side = |id: &str, seen: &HashSet<&str>| {
            if seen.contains(id) {
                id.to_owned()
            } else {
                def(id)
            }
        };
        let from = side(&edge.from, &seen);
        seen.insert(edge.from.as_str());
        let to = side(&edge.to, &seen);
        seen.insert(edge.to.as_str());
        advance(&seen, &mut ptr);
        let label = match relabel {
            Some((target, label)) if target == i => label.map(str::to_owned),
            _ => edge.label.clone(),
        };
        lines.push(format!(
            "{from} {} {to}",
            arrow_with_label(
                edge.stroke,
                edge.head,
                edge.both,
                edge.length,
                edge.id.as_deref(),
                label.as_deref()
            )
        ));
    }
    for id in &order {
        if !seen.contains(id) {
            lines.push(def(id));
            seen.insert(id);
        }
    }
    lines
}

// ---------- Patching ----------

impl Ctx<'_> {
    fn text(&self, s: &Statement) -> &str {
        &self.src[s.range.clone()]
    }
    fn graph_statements(&self) -> impl Iterator<Item = &Statement> {
        self.layout
            .statements
            .iter()
            .filter(|s| s.kind == Kind::Graph)
    }
    fn statement_at(&self, offset: usize) -> Option<&Statement> {
        self.layout
            .statements
            .iter()
            .find(|s| s.range.start <= offset && offset < s.range.end.max(s.range.start + 1))
    }
    fn mentions_of<'m>(&'m self, id: &'m str) -> impl Iterator<Item = &'m Mention> + 'm {
        self.facts.mentions.iter().filter(move |m| {
            m.id == id
                && self
                    .statement_at(m.span.start)
                    .is_some_and(|s| s.kind == Kind::Graph)
        })
    }
    fn patch(&mut self, range: Range<usize>, text: impl Into<String>) {
        self.patches.push((range, text.into()));
    }
    fn indent(&self) -> String {
        let s = self
            .layout
            .statements
            .iter()
            .rev()
            .find(|s| s.kind != Kind::Header && s.depth == 0)
            .or_else(|| {
                self.layout
                    .statements
                    .iter()
                    .find(|s| s.kind != Kind::Header)
            });
        match s {
            Some(s) => source::indent_at(self.src, s.range.start).to_owned(),
            None => "  ".into(),
        }
    }
    fn line_end(&self, offset: usize) -> usize {
        self.src[offset..]
            .find('\n')
            .map_or(self.src.len(), |p| offset + p + 1)
    }
    /// Add a top-level line after the last top-level statement, before any annotation block.
    fn append_line(&mut self, text: &str) {
        let indent = self.indent();
        let anchor = self
            .layout
            .statements
            .iter()
            .rev()
            .find(|s| s.depth == 0)
            .map(|s| s.range.end);
        let at = anchor.map_or(self.src.len(), |end| self.line_end(end));
        let needs_newline = at > 0 && !self.src[..at].ends_with('\n');
        let mut insert = String::new();
        if needs_newline {
            insert.push('\n');
        }
        insert.push_str(&indent);
        insert.push_str(text);
        insert.push('\n');
        self.patch(at..at, insert);
    }
    /// Replace a statement by `lines` (none removes it), keeping the file's line structure.
    fn rewrite_statement(&mut self, s: &Statement, lines: Vec<String>) {
        let src = self.src;
        let shared = source::shares_line(src, &s.range);
        if lines.is_empty() {
            if !shared {
                let start = src[..s.range.start].rfind('\n').map_or(0, |p| p + 1);
                let end = self.line_end(s.range.end);
                let rest = &src[s.range.end..end];
                // A trailing comment on the line survives as its own line.
                if let Some(pos) = rest.find("%%") {
                    self.patch(
                        start..s.range.end + pos,
                        source::indent_at(src, s.range.start).to_owned(),
                    );
                } else {
                    self.patch(start..end, "");
                }
                return;
            }
            let after = &src[s.range.end..];
            let spaces = after.len() - after.trim_start_matches([' ', '\t']).len();
            if after[spaces..].starts_with(';') {
                let tail = &after[spaces + 1..];
                let more = tail.len() - tail.trim_start_matches([' ', '\t']).len();
                self.patch(s.range.start..s.range.end + spaces + 1 + more, "");
            } else {
                let before = &src[..s.range.start];
                let trimmed = before.trim_end_matches([' ', '\t']);
                let start = if trimmed.ends_with(';') {
                    trimmed.len() - 1
                } else {
                    s.range.start
                };
                self.patch(start..s.range.end, "");
            }
            return;
        }
        let joined = if shared {
            lines.join("; ")
        } else {
            lines.join(&format!("\n{}", source::indent_at(src, s.range.start)))
        };
        self.patch(s.range.clone(), joined);
    }

    fn set_label(&mut self, id: &str, label: &str) -> Result<()> {
        let labelled = self
            .mentions_of(id)
            .filter(|m| m.label.is_some() || m.data.is_some())
            .last()
            .cloned();
        if let Some(m) = labelled {
            if let Some((whole, inner)) = &m.label {
                let open = &self.src[whole.start..inner.start];
                let text = if open.ends_with('`') {
                    label.to_owned()
                } else if open.ends_with('"') {
                    entities(label)
                } else {
                    bare_or_quoted(label)
                };
                self.patch(inner.clone(), text);
                return Ok(());
            }
            if let Some(data) = &m.data {
                let body = &self.src[data.clone()];
                let value = format!("\"{}\"", entities(label));
                if let Some(range) = data_value(body, "label") {
                    self.patch(data.start + range.start..data.start + range.end, value);
                } else {
                    let close = data.start + body.trim_end_matches('}').trim_end().len();
                    self.patch(close..close, format!(", label: {value}"));
                }
                return Ok(());
            }
        }
        let first = self.mentions_of(id).next().cloned();
        match first {
            Some(first) => {
                self.patch(
                    first.span.end..first.span.end,
                    format!("[{}]", bare_or_quoted(label)),
                );
            }
            // Only `class`, `style` or `click` lines create this node: define its label on its own line.
            None => self.append_line(&format!("{id}[{}]", bare_or_quoted(label))),
        }
        Ok(())
    }

    /// The statement holding the edge at `index` in diagram order, and its index inside it.
    fn locate_edge(&self, index: usize) -> Result<(Statement, Mini, usize)> {
        let mut seen = 0;
        for s in self.graph_statements() {
            let text = self.text(s);
            if !text.contains(['-', '=', '.', '~', '<']) {
                continue;
            }
            let m = mini(text)?;
            if index < seen + m.edges.len() {
                return Ok((s.clone(), m, index - seen));
            }
            seen += m.edges.len();
        }
        Err(Error::InvalidEdit(
            "the edge is not written in a statement".into(),
        ))
    }

    fn remove_edge(&mut self, edge: &EdgeRef) -> Result<()> {
        let index = edge
            .index(self.diagram)
            .ok_or_else(|| Error::InvalidEdit("no such edge".into()))?;
        let (statement, m, local) = self.locate_edge(index)?;
        let lines = regenerate(&m, &HashSet::new(), |i| i != local, None);
        self.rewrite_statement(&statement, lines);
        self.after_edges_removed(&[index]);
        Ok(())
    }

    fn remove_node(&mut self, id: &str) -> Result<()> {
        let removed: Vec<usize> = self
            .diagram
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.from == id || e.to == id)
            .map(|(i, _)| i)
            .collect();
        let drop: HashSet<String> = HashSet::from([id.to_owned()]);
        let statements: Vec<Statement> = {
            let mut list: Vec<Statement> = vec![];
            for m in self.mentions_of(id) {
                if let Some(s) = self.statement_at(m.span.start) {
                    if !list.iter().any(|l| l.range == s.range) {
                        list.push(s.clone());
                    }
                }
            }
            list
        };
        for s in &statements {
            let m = mini(self.text(s))?;
            let lines = regenerate(&m, &drop, |_| true, None);
            self.rewrite_statement(s, lines);
        }
        let targeted: Vec<Statement> = self
            .layout
            .statements
            .iter()
            .filter(|s| matches!(s.kind, Kind::Class | Kind::Style | Kind::Click))
            .cloned()
            .collect();
        for s in targeted {
            let text = self.text(&s).to_owned();
            let mut words = text.splitn(3, char::is_whitespace);
            let keyword = words.next().unwrap_or("");
            let targets = words.next().unwrap_or("");
            let rest = words.next().unwrap_or("");
            match s.kind {
                Kind::Class => {
                    let list: Vec<&str> = targets.split(',').map(str::trim).collect();
                    if list.contains(&id) {
                        let kept: Vec<&str> = list
                            .into_iter()
                            .filter(|t| *t != id && !t.is_empty())
                            .collect();
                        if kept.is_empty() {
                            self.rewrite_statement(&s, vec![]);
                        } else {
                            self.patch(
                                s.range.clone(),
                                format!("{keyword} {} {rest}", kept.join(","))
                                    .trim_end()
                                    .to_owned(),
                            );
                        }
                    }
                }
                _ if targets == id => self.rewrite_statement(&s, vec![]),
                _ => {}
            }
        }
        // Annotations of the node and of its edges go with them.
        let lines: Vec<Range<usize>> = self
            .layout
            .annotations
            .iter()
            .filter(|a| match &a.target {
                Target::Node(n) => n == id,
                Target::Edge(e) => e.from == id || e.to == id,
                Target::Diagram => false,
            })
            .map(|a| a.line.clone())
            .collect();
        for line in lines {
            self.patch(line, "");
        }
        self.after_edges_removed_except_annotations(&removed);
        Ok(())
    }

    /// Keep `linkStyle` indexes, edge metadata and edge annotations pointing at the right edges.
    fn after_edges_removed(&mut self, removed: &[usize]) {
        for &index in removed {
            let gone = EdgeRef::of(self.diagram, index);
            let lines: Vec<(Range<usize>, EdgeRef, String)> = self
                .layout
                .annotations
                .iter()
                .filter_map(|a| match &a.target {
                    Target::Edge(e)
                        if e.from == gone.from && e.to == gone.to && e.nth >= gone.nth =>
                    {
                        Some((
                            a.line.clone(),
                            e.clone(),
                            self.src[a.line.clone()].to_owned(),
                        ))
                    }
                    _ => None,
                })
                .collect();
            for (line, e, text) in lines {
                if e.nth == gone.nth {
                    self.patch(line, "");
                } else if let Some((_, annotation)) = annotate::parse_line(&text) {
                    let indent = source::indent_at(self.src, line.start).to_owned();
                    let newline = if text.ends_with('\n') { "\n" } else { "" };
                    let target = Target::Edge(EdgeRef {
                        nth: e.nth - 1,
                        ..e
                    });
                    self.patch(
                        line,
                        format!(
                            "{indent}{}{newline}",
                            annotate::format(&target, &annotation)
                        ),
                    );
                }
            }
        }
        self.after_edges_removed_except_annotations(removed);
    }
    fn after_edges_removed_except_annotations(&mut self, removed: &[usize]) {
        let mut removed = removed.to_vec();
        removed.sort_unstable();
        let styles: Vec<Statement> = self
            .layout
            .statements
            .iter()
            .filter(|s| s.kind == Kind::LinkStyle)
            .cloned()
            .collect();
        for s in styles {
            let text = self.text(&s).to_owned();
            let mut words = text.splitn(3, char::is_whitespace);
            let _ = words.next();
            let targets = words.next().unwrap_or("");
            let rest = words.next().unwrap_or("");
            if targets == "default" {
                continue;
            }
            let Ok(indexes) = targets
                .split(',')
                .map(|t| t.trim().parse::<usize>())
                .collect::<std::result::Result<Vec<_>, _>>()
            else {
                continue;
            };
            let kept: Vec<String> = indexes
                .iter()
                .filter(|i| !removed.contains(i))
                .map(|i| (i - removed.iter().filter(|r| *r < i).count()).to_string())
                .collect();
            if kept.is_empty() {
                self.rewrite_statement(&s, vec![]);
            } else if kept.join(",") != targets {
                self.patch(
                    s.range.clone(),
                    format!("linkStyle {} {rest}", kept.join(","))
                        .trim_end()
                        .to_owned(),
                );
            }
        }
        // `e1@{ … }` metadata of a removed edge would otherwise turn into a node named e1.
        let ids: Vec<String> = removed
            .iter()
            .filter_map(|&i| self.diagram.edges[i].id.clone())
            .collect();
        let meta: Vec<Statement> = self
            .graph_statements()
            .filter(|s| {
                ids.iter()
                    .any(|id| self.text(s).starts_with(&format!("{id}@{{")))
            })
            .cloned()
            .collect();
        for s in meta {
            self.rewrite_statement(&s, vec![]);
        }
    }

    fn set_edge_label(&mut self, edge: &EdgeRef, label: Option<&str>) -> Result<()> {
        let index = edge
            .index(self.diagram)
            .ok_or_else(|| Error::InvalidEdit("no such edge".into()))?;
        let (statement, m, local) = self.locate_edge(index)?;
        let base = statement.range.start;
        if m.edges.len() == 1 && m.labels.len() == 1 {
            let (whole, inner) = m.labels[0].clone();
            let (whole, inner) = (
                base + whole.start..base + whole.end,
                base + inner.start..base + inner.end,
            );
            let piped = self.src[whole.clone()].starts_with('|');
            let quoted = self.src[..inner.start].ends_with('"');
            match label {
                Some(label) => {
                    let text = if quoted {
                        entities(label)
                    } else {
                        bare_or_quoted(label)
                    };
                    self.patch(inner, text);
                    return Ok(());
                }
                None if piped => {
                    self.patch(whole, "");
                    return Ok(());
                }
                None => {}
            }
        }
        let lines = regenerate(&m, &HashSet::new(), |_| true, Some((local, label)));
        self.rewrite_statement(&statement, lines);
        Ok(())
    }

    fn annotate(&mut self, target: &Target, annotation: &model::Annotation) {
        let existing = self
            .layout
            .annotations
            .iter()
            .find(|a| &a.target == target)
            .map(|a| a.line.clone());
        let line = annotate::format(target, annotation);
        match existing {
            Some(range) if annotation.is_empty() => self.patch(range, ""),
            Some(range) => {
                let indent = source::indent_at(self.src, range.start).to_owned();
                let newline = if self.src[range.clone()].ends_with('\n') {
                    "\n"
                } else {
                    ""
                };
                self.patch(range, format!("{indent}{line}{newline}"));
            }
            None if annotation.is_empty() => {}
            None => {
                let indent = self.indent();
                match self.layout.annotations.last() {
                    Some(last) => {
                        let at = last.line.end;
                        let lead = if self.src[..at].ends_with('\n') {
                            ""
                        } else {
                            "\n"
                        };
                        self.patch(at..at, format!("{lead}{indent}{line}\n"));
                    }
                    None => {
                        let at = self.src.len();
                        let lead = if self.src.ends_with('\n') {
                            "\n"
                        } else {
                            "\n\n"
                        };
                        self.patch(at..at, format!("{lead}{indent}{line}\n"));
                    }
                }
            }
        }
    }

    fn finish(mut self) -> Result<String> {
        // Deletions that touch (two statements of one line removed together) merge into one;
        // any other overlap means two changes disagree, and the edit is refused.
        self.patches
            .sort_by(|a, b| a.0.start.cmp(&b.0.start).then(a.0.end.cmp(&b.0.end)));
        let mut merged: Vec<(Range<usize>, String)> = vec![];
        for (range, text) in self.patches {
            if let Some((last, last_text)) = merged.last_mut() {
                if range.start < last.end
                    || (range.start == last.end
                        && range.start == range.end
                        && last.start == last.end)
                {
                    if text.is_empty() && last_text.is_empty() {
                        last.end = last.end.max(range.end);
                        continue;
                    }
                    if !(range.is_empty() && range.start == last.end) {
                        return Err(Error::InvalidEdit("overlapping changes".into()));
                    }
                }
            }
            merged.push((range, text));
        }
        let mut out = self.src.to_owned();
        for (range, text) in merged.into_iter().rev() {
            out.replace_range(range, &text);
        }
        Ok(out)
    }
}

/// The value range of `key:` inside `@{ … }` shape data.
fn data_value(body: &str, key: &str) -> Option<Range<usize>> {
    let bytes = body.as_bytes();
    let mut search = 0;
    while let Some(pos) = body[search..].find(key) {
        let at = search + pos;
        search = at + key.len();
        let before_ok = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let rest = &body[search..];
        let colon = rest.len() - rest.trim_start().len();
        if !before_ok || !rest[colon..].starts_with(':') {
            continue;
        }
        let value_start = search + colon + 1;
        let skip = body[value_start..].len() - body[value_start..].trim_start().len();
        let start = value_start + skip;
        if bytes.get(start) == Some(&b'"') {
            let close = body[start + 1..].find('"')?;
            return Some(start..start + 1 + close + 1);
        }
        let end = body[start..]
            .find([',', '}', '\n'])
            .map_or(body.len(), |p| start + p);
        let end = start + body[start..end].trim_end().len();
        return Some(start..end);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_are_canonical() {
        assert_eq!(arrow(Stroke::Normal, Head::Arrow, false, 1), "-->");
        assert_eq!(arrow(Stroke::Normal, Head::None, false, 1), "---");
        assert_eq!(arrow(Stroke::Thick, Head::Arrow, false, 1), "==>");
        assert_eq!(arrow(Stroke::Thick, Head::None, false, 1), "===");
        assert_eq!(arrow(Stroke::Dotted, Head::Arrow, false, 1), "-.->");
        assert_eq!(arrow(Stroke::Dotted, Head::None, false, 2), "-..-");
        assert_eq!(arrow(Stroke::Normal, Head::Circle, false, 1), "--o");
        assert_eq!(arrow(Stroke::Normal, Head::Arrow, true, 1), "<-->");
        assert_eq!(arrow(Stroke::Normal, Head::Arrow, false, 3), "---->");
        assert_eq!(arrow(Stroke::Invisible, Head::None, false, 1), "~~~");
    }

    #[test]
    fn shape_data_values() {
        let body = "@{ shape: cyl, label: \"Event store\" }";
        assert_eq!(&body[data_value(body, "label").unwrap()], "\"Event store\"");
        assert_eq!(&body[data_value(body, "shape").unwrap()], "cyl");
        assert!(data_value("@{ shape: cyl }", "label").is_none());
    }
}
