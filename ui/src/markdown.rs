//! Agent output is untrusted: allow document formatting, never executable HTML or remote images.
use pulldown_cmark::{html, Options, Parser};
use std::collections::{HashMap, HashSet};

pub fn render_markdown(text: &str) -> String {
    let parser = Parser::new_ext(text, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);
    let mut raw = String::new();
    html::push_html(&mut raw, parser);
    let mut attributes: HashMap<&str, HashSet<&str>> = HashMap::new();
    attributes.insert("a", HashSet::from(["href", "title"]));
    attributes.insert("ol", HashSet::from(["start"]));
    ammonia::Builder::default()
        .tags(HashSet::from([
            "p",
            "br",
            "strong",
            "em",
            "del",
            "blockquote",
            "pre",
            "code",
            "ul",
            "ol",
            "li",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "hr",
            "table",
            "thead",
            "tbody",
            "tr",
            "th",
            "td",
            "a",
        ]))
        .generic_attributes(HashSet::new())
        .tag_attributes(attributes)
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .link_rel(Some("noopener noreferrer"))
        .set_tag_attribute_value("a", "target", "_blank")
        .clean(&raw)
        .to_string()
}

/// What a wiki page records about itself in its front matter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrontMatter {
    pub sources: Vec<String>,
    pub commit: Option<String>,
}

/// A piece of a doc: Markdown to render, or a diagram drawn inline.
#[derive(Clone, Debug, PartialEq)]
pub enum Segment {
    Markdown(String),
    /// A line that is only a link to a diagram: `[Title](schematlas:diagram/<id>)`.
    Diagram {
        id: String,
        title: String,
    },
    /// A fenced Mermaid block; `index` counts Mermaid blocks in the file, as diagram ids do.
    Mermaid {
        index: usize,
        source: String,
    },
}

/// Split a doc into its front matter and the pieces to draw.
pub fn segments(markdown: &str) -> (FrontMatter, Vec<Segment>) {
    let mut front = FrontMatter::default();
    let mut body = markdown;
    if let Some(rest) = markdown
        .strip_prefix("---\n")
        .or_else(|| markdown.strip_prefix("---\r\n"))
    {
        if let Some(end) = rest.find("\n---") {
            let yaml = &rest[..end];
            body = rest[end + 4..].trim_start_matches(['\r', '\n']);
            let mut in_sources = false;
            for line in yaml.lines() {
                let trimmed = line.trim();
                if let Some(value) = trimmed.strip_prefix("commit:") {
                    in_sources = false;
                    let value = value.trim().trim_matches(['"', '\'']);
                    front.commit = (!value.is_empty()).then(|| value.to_string());
                } else if let Some(value) = trimmed.strip_prefix("sources:") {
                    let value = value.trim();
                    in_sources = value.is_empty();
                    if let Some(list) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                        front.sources.extend(
                            list.split(',')
                                .map(|s| s.trim().trim_matches(['"', '\'']).to_string())
                                .filter(|s| !s.is_empty()),
                        );
                    }
                } else if let (true, Some(item)) = (in_sources, trimmed.strip_prefix("- ")) {
                    front
                        .sources
                        .push(item.trim().trim_matches(['"', '\'']).to_string());
                } else if !line.starts_with(' ') {
                    in_sources = false;
                }
            }
        }
    }
    let mut out = vec![];
    let mut text = String::new();
    let mut lines = body.lines().peekable();
    let mut mermaid = 0;
    let flush = |text: &mut String, out: &mut Vec<Segment>| {
        if !text.trim().is_empty() {
            out.push(Segment::Markdown(std::mem::take(text)));
        }
        text.clear();
    };
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let fence = ["```", "~~~"].into_iter().find(|f| trimmed.starts_with(f));
        if let Some(fence) = fence {
            let info = trimmed[fence.len()..].trim();
            let mut block = vec![];
            let mut closed = false;
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with(fence) {
                    closed = true;
                    break;
                }
                block.push(inner);
            }
            if info.split_whitespace().next() == Some("mermaid") {
                flush(&mut text, &mut out);
                out.push(Segment::Mermaid {
                    index: mermaid,
                    source: block.join("\n"),
                });
                mermaid += 1;
            } else {
                text.push_str(line);
                text.push('\n');
                for b in &block {
                    text.push_str(b);
                    text.push('\n');
                }
                if closed {
                    text.push_str(fence);
                    text.push('\n');
                }
            }
            continue;
        }
        let link = trimmed
            .trim_end()
            .strip_prefix('[')
            .and_then(|l| l.strip_suffix(')'))
            .and_then(|l| l.split_once("](schematlas:diagram/"));
        if let Some((title, id)) = link.filter(|(t, id)| !t.contains(']') && !id.contains(')')) {
            flush(&mut text, &mut out);
            out.push(Segment::Diagram {
                id: id.to_string(),
                title: title.to_string(),
            });
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    flush(&mut text, &mut out);
    (front, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_formatting_and_drops_scripts_and_images() {
        let html = render_markdown("# Title\n\nSome **bold** text <script>alert(1)</script> ![x](https://e.com/i.png)\n\n[link](https://example.com)");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("<img"));
        assert!(html.contains(r#"href="https://example.com""#));
        assert!(html.contains("noopener"));
        assert!(html.contains(r#"target="_blank""#));
    }
    #[test]
    fn docs_split_into_text_and_inline_diagrams() {
        let doc = "---\ntitle: Payments\nsources:\n  - src/pay.rs\n  - \"src/api/*.rs\"\ncommit: abc123\n---\n# Payments\n\nIntro.\n\n[Flow](schematlas:diagram/.schematlas/diagrams/pay.mmd)\n\n```rust\nfn a() {}\n```\n\n```mermaid\nflowchart LR\n  a --> b\n```\nSee [inline](schematlas:diagram/x.mmd) here.\n";
        let (front, parts) = segments(doc);
        assert_eq!(front.sources, ["src/pay.rs", "src/api/*.rs"]);
        assert_eq!(front.commit.as_deref(), Some("abc123"));
        assert!(matches!(&parts[0], Segment::Markdown(t) if t.starts_with("# Payments")));
        assert_eq!(
            parts[1],
            Segment::Diagram {
                id: ".schematlas/diagrams/pay.mmd".into(),
                title: "Flow".into()
            }
        );
        assert!(
            matches!(&parts[2], Segment::Markdown(t) if t.contains("```rust") && t.trim_end().ends_with("```"))
        );
        assert_eq!(
            parts[3],
            Segment::Mermaid {
                index: 0,
                source: "flowchart LR\n  a --> b".into()
            }
        );
        // A diagram link inside a sentence stays a link.
        assert!(matches!(&parts[4], Segment::Markdown(t) if t.contains("[inline]")));
        assert_eq!(parts.len(), 5);
    }

    #[test]
    fn blocks_javascript_urls() {
        let html = render_markdown("[x](javascript:alert(1))");
        assert!(!html.contains("javascript:"));
    }
}
