//! The layout of a Mermaid source: front matter, statements (with their byte ranges, kinds and
//! nesting), comment and annotation lines, and merman's byte spans for node mentions and labels.
use crate::annotate::{self, Target};
use std::ops::Range;

/// Front matter between leading `---` lines, without the fences.
pub(crate) fn front_matter(src: &str) -> Option<&str> {
    let rest = src.strip_prefix("---")?;
    let rest = rest
        .strip_prefix('\n')
        .or_else(|| rest.strip_prefix("\r\n"))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some(&rest[..offset]);
        }
        offset += line.len();
    }
    None
}
/// Byte offset where the diagram proper starts.
pub(crate) fn body_offset(src: &str) -> usize {
    let Some(block) = front_matter(src) else {
        return 0;
    };
    let start = src.find('\n').map_or(src.len(), |i| i + 1) + block.len();
    src[start..].find('\n').map_or(src.len(), |i| start + i + 1)
}
pub(crate) fn after_front_matter(src: &str) -> &str {
    &src[body_offset(src)..]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Header,
    Graph,
    Subgraph,
    End,
    Direction,
    ClassDef,
    Class,
    Style,
    LinkStyle,
    Click,
    Accessibility,
}

#[derive(Clone, Debug)]
pub(crate) struct Statement {
    pub range: Range<usize>,
    pub kind: Kind,
    /// Subgraph nesting depth at this statement (a `subgraph` line is at its parent's depth).
    pub depth: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct AnnotationLine {
    /// The whole physical line, including its newline.
    pub line: Range<usize>,
    pub target: Target,
}

#[derive(Clone, Debug)]
pub(crate) struct Layout {
    pub statements: Vec<Statement>,
    pub annotations: Vec<AnnotationLine>,
}

fn kind_of(text: &str, first: bool) -> Kind {
    if first {
        return Kind::Header;
    }
    let word = text.split(|c: char| c.is_whitespace()).next().unwrap_or("");
    match word {
        "subgraph" => Kind::Subgraph,
        "end" => Kind::End,
        "direction" => Kind::Direction,
        "classDef" => Kind::ClassDef,
        "class" => Kind::Class,
        "style" => Kind::Style,
        "linkStyle" => Kind::LinkStyle,
        "click" => Kind::Click,
        w if w.starts_with("accTitle") || w.starts_with("accDescr") => Kind::Accessibility,
        _ => Kind::Graph,
    }
}

/// Split the body into statements. A statement ends at a newline or `;` outside quotes, labels
/// and brackets; `%%` starts a comment to the end of the line; `accDescr { … }` spans lines.
pub(crate) fn layout(src: &str) -> Layout {
    let start = body_offset(src);
    let bytes = src.as_bytes();
    let mut statements = vec![];
    let mut annotations = vec![];
    let mut depth = 0usize;
    let mut i = start;
    let mut seen_header = false;
    while i < src.len() {
        // Skip whitespace between statements (but remember line starts for comments).
        let c = bytes[i];
        if c == b'\n' || c == b'\r' || c == b' ' || c == b'\t' || c == b';' {
            i += 1;
            continue;
        }
        if src[i..].starts_with("%%") {
            let line_start = src[..i].rfind('\n').map_or(0, |p| p + 1);
            let line_end = src[i..].find('\n').map_or(src.len(), |p| i + p + 1);
            let only_comment = src[line_start..i].trim().is_empty();
            if only_comment {
                if let Some((target, _)) = annotate::parse_line(&src[i..line_end]) {
                    annotations.push(AnnotationLine {
                        line: line_start..line_end,
                        target,
                    });
                }
            }
            i = line_end;
            continue;
        }
        let begin = i;
        let mut quote = false;
        let mut pipe = false;
        let mut brackets = 0i32;
        let accessibility = src[i..].starts_with("accDescr") && {
            let line_end = src[i..].find('\n').map_or(src.len(), |p| i + p);
            src[i..line_end].trim_end().ends_with('{')
        };
        let mut end = src.len();
        while i < src.len() {
            let c = bytes[i];
            if accessibility {
                if c == b'}' {
                    end = i + 1;
                    i += 1;
                    break;
                }
                i += 1;
                continue;
            }
            if quote {
                if c == b'"' {
                    quote = false;
                }
                i += 1;
                continue;
            }
            match c {
                b'"' => quote = true,
                b'|' if brackets == 0 => pipe = !pipe,
                b'[' | b'(' | b'{' if !pipe => brackets += 1,
                b']' | b')' | b'}' if !pipe => brackets = (brackets - 1).max(0),
                b'\n' | b';' if !pipe && brackets == 0 => {
                    end = i;
                    break;
                }
                b'\n' if pipe => {
                    end = i;
                    break;
                }
                b'%' if !pipe && brackets == 0 && src[i..].starts_with("%%") => {
                    end = i;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
        let text = src[begin..end].trim_end();
        let range = begin..begin + text.len();
        if text.is_empty() {
            continue;
        }
        let kind = kind_of(text, !seen_header);
        seen_header = true;
        match kind {
            Kind::End => {
                depth = depth.saturating_sub(1);
                statements.push(Statement { range, kind, depth });
            }
            Kind::Subgraph => {
                statements.push(Statement { range, kind, depth });
                depth += 1;
            }
            _ => statements.push(Statement { range, kind, depth }),
        }
    }
    Layout {
        statements,
        annotations,
    }
}

/// A node mentioned in the source: its id span and the full text that defines it there
/// (`api[Orders API]:::hot`, `s@{ shape: cyl }`, or just the id).
#[derive(Clone, Debug)]
pub(crate) struct Mention {
    pub id: String,
    pub span: Range<usize>,
    /// Through the label, shape data and class shorthand, if any.
    pub def_end: usize,
    /// The label payload: (whole delimited span, inner text span).
    pub label: Option<(Range<usize>, Range<usize>)>,
    /// `@{ … }` shape data following the id.
    pub data: Option<Range<usize>>,
}

/// Node mentions and edge-label payloads in `src`, from merman's editor facts.
#[derive(Clone, Debug, Default)]
pub(crate) struct Facts {
    pub mentions: Vec<Mention>,
    /// (whole span, inner text span) of edge labels, in source order.
    pub edge_labels: Vec<(Range<usize>, Range<usize>)>,
}

fn matching_brace(src: &str, open: usize) -> Option<usize> {
    let bytes = src.as_bytes();
    let mut depth = 0;
    let mut quote = false;
    for (i, &c) in bytes.iter().enumerate().skip(open) {
        if quote {
            if c == b'"' {
                quote = false;
            }
            continue;
        }
        match c {
            b'"' => quote = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn facts(src: &str) -> Facts {
    let Ok(Some(raw)) =
        super::engine().parse_editor_semantic_facts_with_type_sync("flowchart-v2", src)
    else {
        return Facts::default();
    };
    let mut out = Facts::default();
    let symbols = raw.symbols;
    for (i, s) in symbols.iter().enumerate() {
        match s.detail.as_deref() {
            Some("flowchart node") => {
                let span = s.span.start..s.span.end;
                let label = symbols.get(i + 1).and_then(|next| {
                    (next.detail.as_deref() == Some("flowchart node label")
                        && next.span.start == span.end)
                        .then_some((
                            next.span.start..next.span.end,
                            next.selection.start..next.selection.end,
                        ))
                });
                let mut def_end = label.as_ref().map_or(span.end, |(whole, _)| whole.end);
                let mut data = None;
                if label.is_none() && src[span.end..].starts_with("@{") {
                    if let Some(close) = matching_brace(src, span.end + 1) {
                        data = Some(span.end..close);
                        def_end = close;
                    }
                }
                if src[def_end..].starts_with(":::") {
                    let rest = &src[def_end + 3..];
                    let len = rest
                        .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                        .unwrap_or(rest.len());
                    def_end += 3 + len;
                }
                out.mentions.push(Mention {
                    id: s.name.clone(),
                    span,
                    def_end,
                    label,
                    data,
                });
            }
            Some("flowchart edge label") => {
                out.edge_labels
                    .push((s.span.start..s.span.end, s.selection.start..s.selection.end));
            }
            _ => {}
        }
    }
    out
}

/// Indentation of the line containing `offset`.
pub(crate) fn indent_at(src: &str, offset: usize) -> &str {
    let start = src[..offset].rfind('\n').map_or(0, |p| p + 1);
    let line = &src[start..];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}
/// Whether the statement shares its physical line with other statements.
pub(crate) fn shares_line(src: &str, range: &Range<usize>) -> bool {
    let start = src[..range.start].rfind('\n').map_or(0, |p| p + 1);
    let end = src[range.end..]
        .find('\n')
        .map_or(src.len(), |p| range.end + p);
    let before = src[start..range.start].trim();
    let after = src[range.end..end].trim();
    let after = after.split("%%").next().unwrap_or("").trim();
    !(before.is_empty() || before == ";") || !(after.is_empty() || after == ";")
}
