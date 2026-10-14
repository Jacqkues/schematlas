//! Mermaid blocks inside Markdown, so a diagram in a README is as editable as a `.mmd` file.
use std::ops::Range;

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    /// Position among the document's Mermaid blocks.
    pub index: usize,
    /// Byte range of the block's content, between the fences.
    pub range: Range<usize>,
    /// The nearest heading above the block, a readable name for it.
    pub heading: Option<String>,
    /// Indentation of the opening fence, removed from content lines when parsing.
    pub indent: usize,
}
impl Block {
    /// The Mermaid source, with the fence's indentation removed.
    pub fn source(&self, doc: &str) -> String {
        doc[self.range.clone()]
            .split_inclusive('\n')
            .map(|line| {
                let strip = line.len() - line.trim_start_matches(' ').len();
                &line[strip.min(self.indent)..]
            })
            .collect()
    }
}

struct Fence {
    char: char,
    len: usize,
    indent: usize,
    mermaid: bool,
    content_start: usize,
}

fn opening(line: &str) -> Option<(char, usize, usize, String)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 12 {
        return None;
    }
    let rest = &line[indent..];
    let char = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let len = rest.chars().take_while(|c| *c == char).count();
    if len < 3 {
        return None;
    }
    let info = rest[len..].trim();
    if char == '`' && info.contains('`') {
        return None;
    }
    Some((
        char,
        len,
        indent,
        info.split_whitespace().next().unwrap_or("").to_owned(),
    ))
}

/// Every ```` ```mermaid ```` or `~~~mermaid` block, skipping ones nested inside other fences.
pub fn mermaid_blocks(doc: &str) -> Vec<Block> {
    let mut blocks = vec![];
    let mut heading: Option<String> = None;
    let mut open: Option<Fence> = None;
    let mut offset = 0;
    for line in doc.split_inclusive('\n') {
        let start = offset;
        offset += line.len();
        let text = line.trim_end_matches(['\n', '\r']);
        match &open {
            Some(fence) => {
                let trimmed = text.trim_start_matches(' ');
                let closes = trimmed.chars().take_while(|c| *c == fence.char).count() >= fence.len
                    && trimmed.trim_start_matches(fence.char).trim().is_empty();
                if closes {
                    if fence.mermaid {
                        blocks.push(Block {
                            index: blocks.len(),
                            range: fence.content_start..start,
                            heading: heading.clone(),
                            indent: fence.indent,
                        });
                    }
                    open = None;
                }
            }
            None => {
                if let Some((char, len, indent, info)) = opening(text) {
                    open = Some(Fence {
                        char,
                        len,
                        indent,
                        mermaid: info == "mermaid",
                        content_start: offset,
                    });
                } else if let Some(title) = text.trim_start().strip_prefix('#') {
                    let title = title.trim_start_matches('#');
                    if title.starts_with(' ') || title.is_empty() {
                        let title = title.trim().trim_end_matches('#').trim();
                        if !title.is_empty() {
                            heading = Some(title.to_owned());
                        }
                    }
                }
            }
        }
    }
    // An unterminated fence runs to the end of the document, as in CommonMark.
    if let Some(fence) = open.filter(|f| f.mermaid) {
        blocks.push(Block {
            index: blocks.len(),
            range: fence.content_start..doc.len(),
            heading,
            indent: fence.indent,
        });
    }
    blocks
}

/// The document with one block's content replaced, re-indented to match its fence.
pub fn replace_block(doc: &str, block: &Block, source: &str) -> String {
    let pad = " ".repeat(block.indent);
    let mut content = String::new();
    for line in source.split_inclusive('\n') {
        if !line.trim().is_empty() {
            content.push_str(&pad);
        }
        content.push_str(line);
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    format!(
        "{}{}{}",
        &doc[..block.range.start],
        content,
        &doc[block.range.end..]
    )
}
