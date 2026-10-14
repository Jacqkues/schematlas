//! `%% @schematlas` annotation lines.
//!
//! ```text
//! %% @schematlas diagram title="Checkout" kind=architecture
//! %% @schematlas node api code=src/api.rs:12-80 note="Validates carts" tags=http,core
//! %% @schematlas edge api db 1 code=src/db.rs:40
//! ```
//!
//! One physical line per target. Values are bare words or double-quoted strings with `\"`, `\\`
//! and `\n` escapes. Unknown keys are kept, so files written by newer versions survive.
use crate::model::{Annotation, CodeRef, EdgeRef};
use serde::{Deserialize, Serialize};

pub const MARKER: &str = "@schematlas";

/// What an annotation line describes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    Diagram,
    Node(String),
    Edge(EdgeRef),
}

fn tokens(text: &str) -> Option<Vec<String>> {
    let mut out = vec![];
    let mut chars = text.chars().peekable();
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        let Some(&first) = chars.peek() else {
            return Some(out);
        };
        let _ = first;
        let mut token = String::new();
        // A token is `key`, `key=value` or `key="quoted value"`; quotes may only follow `=`.
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            chars.next();
            if c == '"' {
                if !token.ends_with('=') {
                    return None;
                }
                token.push('\u{1}');
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => match chars.next()? {
                            'n' => token.push('\n'),
                            other => token.push(other),
                        },
                        other => token.push(other),
                    }
                }
                if chars.peek().is_some_and(|c| !c.is_whitespace()) {
                    return None;
                }
                break;
            }
            token.push(c);
        }
        out.push(token);
    }
}

/// The target and annotation of one Mermaid comment line (`%% @schematlas …`), or None when the
/// line is not a well-formed annotation.
pub fn parse_line(line: &str) -> Option<(Target, Annotation)> {
    parse_comment(line, "%%")
}

/// Like [`parse_line`] for any line-comment syntax (`//` in DBML).
pub fn parse_comment(line: &str, comment: &str) -> Option<(Target, Annotation)> {
    let rest = line.trim_start().strip_prefix(comment)?.trim_start();
    let rest = rest.strip_prefix(MARKER)?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let mut words = tokens(rest)?.into_iter();
    let target = match words.next()?.as_str() {
        "diagram" => Target::Diagram,
        "node" => {
            let id = words.next()?;
            if id.contains('=') {
                return None;
            }
            Target::Node(id)
        }
        "edge" => {
            let from = words.next()?;
            let to = words.next()?;
            if from.contains('=') || to.contains('=') {
                return None;
            }
            let mut rest: Vec<String> = words.collect();
            let nth = match rest.first().and_then(|w| w.parse::<usize>().ok()) {
                Some(n) => {
                    rest.remove(0);
                    n
                }
                None => 0,
            };
            words = rest.into_iter();
            Target::Edge(EdgeRef { from, to, nth })
        }
        _ => return None,
    };
    let mut annotation = Annotation::default();
    for word in words {
        let (key, value) = match word.split_once('=') {
            Some((key, value)) => (key.to_owned(), value.trim_start_matches('\u{1}').to_owned()),
            None => (word, String::new()),
        };
        if key.is_empty() {
            return None;
        }
        match key.as_str() {
            "code" => {
                let code = CodeRef::parse(&value);
                // An unsafe or malformed reference is dropped, never trusted.
                if code.validate().is_ok() {
                    annotation.code.push(code);
                }
            }
            "note" => annotation.note = Some(value),
            "title" => annotation.title = Some(value),
            "kind" => annotation.kind = Some(value),
            "tags" => {
                annotation.tags = value
                    .split(',')
                    .map(|t| t.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .collect()
            }
            _ => {
                annotation.extra.insert(key, value);
            }
        }
    }
    Some((target, annotation))
}

fn value(text: &str) -> String {
    let bare = !text.is_empty()
        && text
            .chars()
            .all(|c| !c.is_whitespace() && !matches!(c, '"' | '\\' | '\u{1}'));
    if bare {
        return text.to_owned();
    }
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// The single Mermaid annotation line for a target, without indentation or newline.
pub fn format(target: &Target, annotation: &Annotation) -> String {
    format_comment("%%", target, annotation)
}

/// Like [`format`] for any line-comment syntax.
pub fn format_comment(comment: &str, target: &Target, annotation: &Annotation) -> String {
    let mut line = format!("{comment} {MARKER} ");
    match target {
        Target::Diagram => line.push_str("diagram"),
        Target::Node(id) => {
            line.push_str("node ");
            line.push_str(id);
        }
        Target::Edge(edge) => {
            line.push_str(&format!("edge {} {}", edge.from, edge.to));
            if edge.nth > 0 {
                line.push_str(&format!(" {}", edge.nth));
            }
        }
    }
    let mut pairs: Vec<(String, String)> = vec![];
    if let Some(title) = &annotation.title {
        pairs.push(("title".into(), title.clone()));
    }
    if let Some(kind) = &annotation.kind {
        pairs.push(("kind".into(), kind.clone()));
    }
    for code in &annotation.code {
        pairs.push(("code".into(), code.to_string()));
    }
    if let Some(note) = &annotation.note {
        pairs.push(("note".into(), note.clone()));
    }
    if !annotation.tags.is_empty() {
        pairs.push(("tags".into(), annotation.tags.join(",")));
    }
    for (key, v) in &annotation.extra {
        pairs.push((key.clone(), v.clone()));
    }
    for (key, v) in pairs {
        line.push(' ');
        line.push_str(&key);
        line.push('=');
        line.push_str(&value(&v));
    }
    line
}
