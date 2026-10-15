//! Mermaid labels as people write them: `<br>` line breaks, `<b>`/`<i>` tags, and markdown
//! strings (`**bold**`, `*italic*`, real newlines). They are drawn as plain text runs, never as
//! HTML, so a label can format but cannot inject markup.

/// A piece of a line with its emphasis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

/// Height of one label line on a card (13px text at 1.3 line height).
pub const LINE_HEIGHT: f64 = 17.0;

/// Split at `<br>`, `<br/>`, `<br />` (any case) and newlines.
fn raw_lines(label: &str) -> Vec<String> {
    let mut lines = vec![];
    let mut current = String::new();
    let lower = label.to_ascii_lowercase();
    let mut i = 0;
    let bytes = label.as_bytes();
    while i < label.len() {
        if lower[i..].starts_with("<br") {
            if let Some(end) = lower[i..].find('>') {
                let inside = &lower[i + 3..i + end];
                if inside.trim().trim_end_matches('/').trim().is_empty() {
                    lines.push(std::mem::take(&mut current));
                    i += end + 1;
                    continue;
                }
            }
        }
        // Agents often write `\n` in labels, meaning a line break.
        if label[i..].starts_with("\\n") {
            lines.push(std::mem::take(&mut current));
            i += 2;
            continue;
        }
        if bytes[i] == b'\n' {
            lines.push(std::mem::take(&mut current));
            i += 1;
            continue;
        }
        let ch = label[i..].chars().next().unwrap_or(' ');
        if ch != '\r' {
            current.push(ch);
        }
        i += ch.len_utf8();
    }
    lines.push(current);
    lines
}

/// One line as runs: `<b>`/`<strong>` and `**` or `__` make bold, `<i>`/`<em>` and `*` or `_`
/// (between word boundaries) make italic; other tags are dropped, their text kept.
fn runs(line: &str) -> Vec<Run> {
    let mut out: Vec<Run> = vec![];
    let (mut bold, mut italic) = (0i32, 0i32);
    let mut text = String::new();
    let push = |out: &mut Vec<Run>, text: &mut String, bold: i32, italic: i32| {
        if text.is_empty() {
            return;
        }
        let run = Run {
            text: std::mem::take(text),
            bold: bold > 0,
            italic: italic > 0,
        };
        match out.last_mut() {
            Some(last) if last.bold == run.bold && last.italic == run.italic => {
                last.text.push_str(&run.text)
            }
            _ => out.push(run),
        }
    };
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let opens_tag = chars
            .get(i + 1)
            .is_some_and(|n| n.is_ascii_alphabetic() || *n == '/');
        if c == '<' && opens_tag {
            if let Some(len) = chars[i..].iter().position(|c| *c == '>') {
                let tag: String = chars[i + 1..i + len]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase();
                let (closing, name) = match tag.strip_prefix('/') {
                    Some(name) => (true, name.trim().to_string()),
                    None => (
                        false,
                        tag.split_whitespace()
                            .next()
                            .unwrap_or("")
                            .trim_end_matches('/')
                            .to_string(),
                    ),
                };
                let delta = if closing { -1 } else { 1 };
                let known = matches!(
                    name.as_str(),
                    "b" | "strong"
                        | "i"
                        | "em"
                        | "u"
                        | "span"
                        | "small"
                        | "sub"
                        | "sup"
                        | "code"
                        | "font"
                );
                if known || name.chars().all(|c| c.is_ascii_alphanumeric()) && !name.is_empty() {
                    push(&mut out, &mut text, bold, italic);
                    match name.as_str() {
                        "b" | "strong" => bold = (bold + delta).max(0),
                        "i" | "em" => italic = (italic + delta).max(0),
                        _ => {}
                    }
                    i += len + 1;
                    continue;
                }
            }
        }
        let double = i + 1 < chars.len() && chars[i + 1] == c;
        if (c == '*' || c == '_') && double {
            push(&mut out, &mut text, bold, italic);
            bold = if bold > 0 { 0 } else { 1 };
            i += 2;
            continue;
        }
        if c == '*' || c == '_' {
            // Emphasis only where it reads as markup, not inside words like snake_case.
            let before = i == 0 || !chars[i - 1].is_alphanumeric();
            let after = i + 1 >= chars.len() || !chars[i + 1].is_alphanumeric();
            if (italic == 0 && before && !after) || (italic > 0 && after) {
                push(&mut out, &mut text, bold, italic);
                italic = if italic > 0 { 0 } else { 1 };
                i += 1;
                continue;
            }
        }
        text.push(c);
        i += 1;
    }
    push(&mut out, &mut text, bold, italic);
    out
}

/// A label as lines of runs, without empty trailing lines.
pub fn lines(label: &str) -> Vec<Vec<Run>> {
    let label = label.trim();
    // Markdown strings arrive with their backticks when a writer kept them.
    let label = label
        .strip_prefix('`')
        .and_then(|l| l.strip_suffix('`'))
        .unwrap_or(label);
    let mut out: Vec<Vec<Run>> = raw_lines(label).iter().map(|l| runs(l.trim())).collect();
    while out.len() > 1 && out.last().is_some_and(Vec::is_empty) {
        out.pop();
    }
    out
}

/// The label as plain text, lines joined by spaces: for search, titles and screen readers.
pub fn plain(label: &str) -> String {
    lines(label)
        .iter()
        .map(|line| line.iter().map(|r| r.text.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

/// How many lines the label takes.
pub fn line_count(label: &str) -> usize {
    lines(label).len().max(1)
}

/// Most lines a card shows of its label; longer labels end with an ellipsis.
pub const MAX_LINES: usize = 5;
/// Room for label text on a card: its width less padding, icon and gap.
const TEXT_WIDTH: f64 = 150.0;

/// Rough width of text at 13px semibold, from character classes. A little generous, so a
/// card is never shorter than its text.
fn text_width(text: &str) -> f64 {
    text.chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | 'I' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 3.6,
            'f' | 't' | 'r' | ' ' | '(' | ')' | '[' | ']' | '/' | '-' => 4.9,
            'm' | 'w' | 'M' | 'W' | '@' => 11.0,
            c if c.is_ascii_uppercase() || c.is_ascii_digit() => 8.4,
            c if c.is_ascii() => 7.3,
            // Wide scripts and emoji take about a full em.
            _ => 13.0,
        })
        .sum()
}

/// Lines the label occupies on a card once long lines wrap, at most [`MAX_LINES`].
pub fn shown_lines(label: &str) -> usize {
    let wrapped: usize = lines(label)
        .iter()
        .map(|line| {
            let text: String = line.iter().map(|r| r.text.as_str()).collect();
            ((text_width(&text) / TEXT_WIDTH).ceil() as usize).max(1)
        })
        .sum();
    wrapped.clamp(1, MAX_LINES)
}

/// The label in the inline editor: one text line per label line, markup kept.
pub fn editable(label: &str) -> String {
    raw_lines(label.trim()).join("\n")
}

/// What the editor's text becomes in the file: line breaks as `<br>`, which every Mermaid
/// renderer understands.
pub fn stored(text: &str) -> String {
    text.trim()
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("<br>")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Vec<Run>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.iter().map(|r| r.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn breaks_split_lines_in_every_spelling() {
        let l = lines("Line one<br>Line two<br />Line three<BR>Four<br/>Five");
        assert_eq!(
            text(&l),
            ["Line one", "Line two", "Line three", "Four", "Five"]
        );
        assert_eq!(text(&lines("**Bold** and *italic*\nsecond line")).len(), 2);
        assert_eq!(line_count("one"), 1);
        assert_eq!(plain("Orders API<br/>(Rust)"), "Orders API (Rust)");
        assert_eq!(
            text(&lines("main.py\\nuvicorn launcher")),
            ["main.py", "uvicorn launcher"]
        );
    }

    #[test]
    fn emphasis_from_tags_and_markdown() {
        let l = lines("<b>Bold</b> <i>it</i> #1");
        assert_eq!(
            l[0],
            [
                Run {
                    text: "Bold".into(),
                    bold: true,
                    italic: false
                },
                Run {
                    text: " ".into(),
                    bold: false,
                    italic: false
                },
                Run {
                    text: "it".into(),
                    bold: false,
                    italic: true
                },
                Run {
                    text: " #1".into(),
                    bold: false,
                    italic: false
                },
            ]
        );
        let l = lines("**Bold** and *italic*");
        assert_eq!(
            l[0][0],
            Run {
                text: "Bold".into(),
                bold: true,
                italic: false
            }
        );
        assert_eq!(
            l[0][2],
            Run {
                text: "italic".into(),
                bold: false,
                italic: true
            }
        );
        // Words with underscores stay as they are.
        assert_eq!(plain("order_items table"), "order_items table");
        // Unknown markup never reaches the page as HTML, only its text.
        assert_eq!(plain("<script>x</script>y"), "xy");
        assert_eq!(plain("a < b > c"), "a < b > c");
    }

    #[test]
    fn long_labels_count_their_wrapped_lines() {
        assert_eq!(shown_lines("API"), 1);
        assert_eq!(shown_lines("Orders API<br>(Rust)"), 2);
        // One long line wraps, and a path without spaces wraps anywhere.
        assert!(shown_lines("Validates carts, prices orders and coordinates payment") >= 3);
        assert!(shown_lines("services/api/checkout/handlers/payment_intent.rs") >= 2);
        // Never more than a card shows.
        assert_eq!(shown_lines(&"word ".repeat(200)), MAX_LINES);
    }

    #[test]
    fn editing_round_trips_line_breaks() {
        assert_eq!(editable("Orders API<br/>(Rust)"), "Orders API\n(Rust)");
        assert_eq!(stored("Orders API\n(Rust)\n"), "Orders API<br>(Rust)");
        assert_eq!(stored(&editable("a<br>b")), "a<br>b");
    }
}
