//! The `%% @schematlas` annotation lines: invisible to Mermaid and GitHub, read by Schematlas.
mod support;
use schematlas_parser::annotate::{self, Target};
use schematlas_parser::mermaid;
use schematlas_parser::model::{Annotation, CodeRef, EdgeRef};
use support::fixture;

#[test]
fn annotations_attach_to_nodes_edges_and_the_diagram() {
    let d = mermaid::parse(&fixture("10-annotated.mmd"))
        .unwrap()
        .diagram;
    assert_eq!(d.annotation.title.as_deref(), Some("Checkout architecture"));
    assert_eq!(d.annotation.kind.as_deref(), Some("architecture"));
    let api = d.nodes.iter().find(|n| n.id == "api").unwrap();
    assert_eq!(
        api.annotation.code,
        [
            CodeRef::parse("src/api/checkout.rs:18-64"),
            CodeRef::parse("src/api/mod.rs")
        ]
    );
    assert_eq!(api.annotation.code[0].line, Some(18));
    assert_eq!(api.annotation.code[0].end_line, Some(64));
    assert_eq!(
        api.annotation.note.as_deref(),
        Some("Validates carts and prices orders.")
    );
    assert_eq!(api.annotation.tags, ["http", "core"]);
    let db = d.nodes.iter().find(|n| n.id == "db").unwrap();
    assert_eq!(db.annotation.kind.as_deref(), Some("store"));
    assert_eq!(
        db.annotation.note.as_deref(),
        Some("SQLite in dev, \"Postgres\" in prod.\nRead replica for reports.")
    );
    let edge = &d.edges[0];
    assert_eq!(
        edge.annotation.code,
        [CodeRef::parse("src/api/orders.rs:40")]
    );
    // Keys from newer versions survive untouched.
    assert_eq!(
        edge.annotation.extra.get("future-key").map(String::as_str),
        Some("kept")
    );
}

#[test]
fn lines_round_trip_through_format_and_parse() {
    let mut annotation = Annotation {
        code: vec![CodeRef::parse("src/a b/x.rs:1-2"), CodeRef::parse("lib.rs")],
        note: Some("Line one\nline \"two\" with \\ backslash".into()),
        tags: vec!["a".into(), "b-c".into()],
        kind: Some("service".into()),
        ..Annotation::default()
    };
    annotation
        .extra
        .insert("x-owner".into(), "team payments".into());
    for target in [
        Target::Diagram,
        Target::Node("user.service".into()),
        Target::Edge(EdgeRef {
            from: "a".into(),
            to: "b".into(),
            nth: 2,
        }),
    ] {
        let line = annotate::format(&target, &annotation);
        assert!(line.starts_with("%% @schematlas "), "{line}");
        assert!(!line.contains('\n'), "one physical line: {line}");
        let (parsed_target, parsed) = annotate::parse_line(&line).unwrap();
        assert_eq!(parsed_target, target);
        assert_eq!(parsed, annotation);
    }
}

#[test]
fn malformed_and_foreign_comments_are_ignored() {
    for line in [
        "%% just a comment",
        "%%{init: {\"theme\": \"dark\"}}%%",
        "%% @schematlas",
        "%% @schematlas node",
        "%% @schematlas wat x",
        "%% @schematlas node a note=\"unterminated",
        "%%@schematlasnode a",
    ] {
        assert!(annotate::parse_line(line).is_none(), "{line}");
    }
    // Leading indentation is fine, unknown flags without values are kept as empty extras.
    let (_, a) = annotate::parse_line("    %%   @schematlas node a  flag  code=x.rs").unwrap();
    assert_eq!(a.code, [CodeRef::parse("x.rs")]);
    assert_eq!(a.extra.get("flag").map(String::as_str), Some(""));
}

#[test]
fn code_references_stay_inside_the_repository() {
    for bad in ["../x", "/etc/passwd", "a//b", "C:/x", "a\\b"] {
        assert!(CodeRef::parse(bad).validate().is_err(), "{bad}");
    }
    let r = CodeRef::parse("./src/main.rs#L5-L20");
    assert_eq!(
        (r.path.as_str(), r.line, r.end_line),
        ("src/main.rs", Some(5), Some(20))
    );
    assert_eq!(r.to_string(), "src/main.rs:5-20");
    // An invalid reference in a file is dropped from the model rather than failing the diagram.
    let d =
        mermaid::parse("flowchart LR\n  a\n  %% @schematlas node a code=../../secret code=ok.rs")
            .unwrap()
            .diagram;
    assert_eq!(d.nodes[0].annotation.code, [CodeRef::parse("ok.rs")]);
}
