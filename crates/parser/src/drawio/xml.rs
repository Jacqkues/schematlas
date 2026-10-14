//! A byte-accurate element index over an XML string, so edits can replace one start tag or one
//! element and leave every other byte as the author's tool wrote it.
use crate::error::{Error, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::ops::Range;

#[derive(Clone, Debug)]
pub(crate) struct Element {
    pub name: String,
    /// From `<` to the end of the start tag (the whole element when self-closing).
    pub tag: Range<usize>,
    /// The whole element, start tag to end tag.
    pub span: Range<usize>,
    /// Attributes in document order, unescaped.
    pub attrs: Vec<(String, String)>,
    pub children: Vec<usize>,
    pub empty: bool,
    /// Raw text content directly inside the element (the compressed page payload).
    pub text: Option<Range<usize>>,
}
impl Element {
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

fn attributes(e: &BytesStart, reader: &Reader<&[u8]>) -> Result<Vec<(String, String)>> {
    let mut out = vec![];
    for attr in e.attributes() {
        let attr = attr.map_err(|err| Error::Parse {
            message: format!("bad attribute: {err}"),
            offset: None,
        })?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .decode_and_unescape_value(reader.decoder())
            .map_err(|err| Error::Parse {
                message: format!("bad attribute value: {err}"),
                offset: None,
            })?
            .into_owned();
        out.push((key, value));
    }
    Ok(out)
}

pub(crate) fn index(xml: &str) -> Result<Vec<Element>> {
    let mut reader = Reader::from_reader(xml.as_bytes());
    reader.config_mut().trim_text(false);
    let mut elements: Vec<Element> = vec![];
    let mut stack: Vec<usize> = vec![];
    loop {
        let before = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|e| Error::Parse {
            message: format!("not well-formed XML: {e}"),
            offset: Some(reader.error_position() as usize),
        })?;
        let after = reader.buffer_position() as usize;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let empty = matches!(event, Event::Empty(_));
                let index = elements.len();
                let parent = stack.last().copied();
                elements.push(Element {
                    name: String::from_utf8_lossy(e.name().as_ref()).into_owned(),
                    tag: before..after,
                    span: before..after,
                    attrs: attributes(e, &reader)?,
                    children: vec![],
                    empty,
                    text: None,
                });
                if let Some(p) = parent {
                    elements[p].children.push(index);
                }
                if !empty {
                    stack.push(index);
                }
            }
            Event::End(_) => {
                let index = stack.pop().ok_or_else(|| Error::Parse {
                    message: "unbalanced XML".into(),
                    offset: Some(before),
                })?;
                elements[index].span.end = after;
            }
            Event::Text(_) | Event::CData(_) => {
                if let Some(&top) = stack.last() {
                    let range = match elements[top].text.take() {
                        Some(existing) => existing.start..after,
                        None => before..after,
                    };
                    elements[top].text = Some(range);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !stack.is_empty() {
        return Err(Error::Parse {
            message: "unclosed XML element".into(),
            offset: None,
        });
    }
    Ok(elements)
}

pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            other => out.push(other),
        }
    }
    out
}

/// A start tag with the given attributes, in order.
pub(crate) fn start_tag(name: &str, attrs: &[(String, String)], empty: bool) -> String {
    let mut tag = format!("<{name}");
    for (key, value) in attrs {
        tag.push_str(&format!(" {key}=\"{}\"", escape(value)));
    }
    tag.push_str(if empty { " />" } else { ">" });
    tag
}
