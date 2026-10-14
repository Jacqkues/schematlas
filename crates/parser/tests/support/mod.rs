//! Shared helpers for the parser test suites.
#![allow(dead_code)]
use schematlas_parser::annotate::Target;
use schematlas_parser::mermaid;
use schematlas_parser::model::{self, Annotation, CodeRef, Diagram, EdgeRef, Edit};
use std::path::PathBuf;

pub fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mermaid")
}
pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_dir().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}
/// Every `.mmd` fixture, sorted, as (file name, source).
pub fn fixtures() -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = std::fs::read_dir(fixture_dir())
        .unwrap()
        .filter_map(|e| {
            let path = e.ok()?.path();
            (path.extension()? == "mmd").then(|| {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                let src = std::fs::read_to_string(&path).unwrap();
                (name, src)
            })
        })
        .collect();
    all.sort();
    all
}
/// Lines of `after` that are not in `before`, and vice versa: how much an edit touched.
pub fn changed_lines(before: &str, after: &str) -> (Vec<String>, Vec<String>) {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    let removed = a
        .iter()
        .filter(|l| !b.contains(l))
        .map(|l| l.to_string())
        .collect();
    let added = b
        .iter()
        .filter(|l| !a.contains(l))
        .map(|l| l.to_string())
        .collect();
    (removed, added)
}
/// A tiny deterministic generator, so randomized suites are reproducible without a dependency.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

/// The core promise: applying an edit to the text and re-reading it gives the same diagram as
/// applying the same edit to the model.
pub fn check(name: &str, src: &str, edit: &Edit) -> Option<String> {
    let before = mermaid::parse(src).unwrap().diagram;
    let expected = match model::apply(&before, edit) {
        Ok(d) => d,
        Err(_) => {
            assert!(
                mermaid::apply(src, edit).is_err(),
                "{name}: text accepted what the model refused: {edit:?}"
            );
            return None;
        }
    };
    let text = mermaid::apply(src, edit).unwrap_or_else(|e| panic!("{name}: {edit:?}: {e}\n{src}"));
    let after = mermaid::parse(&text)
        .unwrap_or_else(|e| panic!("{name}: {edit:?} produced invalid Mermaid: {e}\n{text}"));
    assert!(
        after.diagram.same_as(&expected),
        "{name}: {edit:?}\n--- before\n{src}\n--- after\n{text}\n--- expected\n{expected:#?}\n--- got\n{:#?}",
        after.diagram
    );
    Some(text)
}

pub fn every_edit(d: &Diagram) -> Vec<Edit> {
    let mut edits = vec![];
    for n in &d.nodes {
        edits.push(Edit::SetLabel {
            id: n.id.clone(),
            label: format!("{} (renamed)", n.label),
        });
        edits.push(Edit::RemoveNode { id: n.id.clone() });
        let a = Annotation {
            code: vec![CodeRef::parse("src/lib.rs:1-3")],
            note: Some(format!("About {}", n.id)),
            ..Annotation::default()
        };
        edits.push(Edit::Annotate {
            target: Target::Node(n.id.clone()),
            annotation: a,
        });
    }
    for (i, e) in d.edges.iter().enumerate() {
        let nth = d.edges[..i]
            .iter()
            .filter(|x| x.from == e.from && x.to == e.to)
            .count();
        let r = EdgeRef {
            from: e.from.clone(),
            to: e.to.clone(),
            nth,
        };
        edits.push(Edit::RemoveEdge(r.clone()));
        edits.push(Edit::SetEdgeLabel {
            edge: r.clone(),
            label: Some("relabelled".into()),
        });
        edits.push(Edit::SetEdgeLabel {
            edge: r,
            label: None,
        });
    }
    if let (Some(first), Some(last)) = (d.nodes.first(), d.nodes.last()) {
        edits.push(Edit::AddEdge {
            from: last.id.clone(),
            to: first.id.clone(),
            label: Some("back".into()),
        });
    }
    edits.push(Edit::AddNode {
        id: "fresh".into(),
        label: "Fresh node".into(),
        shape: Some("cylinder".into()),
    });
    edits
}

/// draw.io's own page compression, written independently of the crate for its tests.
pub fn drawio_compress(xml: &str) -> String {
    use base64::Engine as _;
    use std::io::Write;
    let mut encoded = String::new();
    for byte in xml.bytes() {
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
            | b')' => encoded.push(byte as char),
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    let mut encoder = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::default());
    encoder.write_all(encoded.as_bytes()).unwrap();
    base64::engine::general_purpose::STANDARD.encode(encoder.finish().unwrap())
}
