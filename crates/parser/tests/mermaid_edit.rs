//! Surgical edits: each change rewrites as little of the author's text as possible.
mod support;
use schematlas_parser::annotate::Target;
use schematlas_parser::mermaid;
use schematlas_parser::model::{Annotation, CodeRef, EdgeRef, Edit};
use support::{changed_lines, fixture};

fn edit(src: &str, edit: Edit) -> String {
    mermaid::apply(src, &edit).unwrap_or_else(|e| panic!("{edit:?}: {e}"))
}
fn edge(from: &str, to: &str) -> EdgeRef {
    EdgeRef {
        from: from.into(),
        to: to.into(),
        nth: 0,
    }
}

#[test]
fn renaming_replaces_only_the_label_text() {
    let src = fixture("01-basic.mmd");
    let out = edit(
        &src,
        Edit::SetLabel {
            id: "api".into(),
            label: "Checkout API".into(),
        },
    );
    assert_eq!(
        out,
        "flowchart LR\n  api[Checkout API] --> db[(Commerce DB)]\n"
    );
    // The shape delimiters are kept.
    let out = edit(
        &src,
        Edit::SetLabel {
            id: "db".into(),
            label: "Orders store".into(),
        },
    );
    assert_eq!(
        out,
        "flowchart LR\n  api[Orders API] --> db[(Orders store)]\n"
    );
}

#[test]
fn renaming_a_bare_node_adds_a_label_at_its_first_mention() {
    let src = "flowchart LR\n  a --> b\n  b --> c\n";
    let out = edit(
        src,
        Edit::SetLabel {
            id: "b".into(),
            label: "Bee".into(),
        },
    );
    assert_eq!(out, "flowchart LR\n  a --> b[Bee]\n  b --> c\n");
}

#[test]
fn labels_with_mermaid_syntax_characters_are_quoted_and_escaped() {
    let src = "flowchart LR\n  a[Plain] --> b\n";
    let out = edit(
        src,
        Edit::SetLabel {
            id: "a".into(),
            label: "Say \"hi\" [now] | later".into(),
        },
    );
    assert_eq!(
        out,
        "flowchart LR\n  a[\"Say #quot;hi#quot; [now] | later\"] --> b\n"
    );
    let back = mermaid::parse(&out).unwrap().diagram;
    assert_eq!(back.nodes[0].label, "Say \"hi\" [now] | later");
    // Already quoted labels stay quoted.
    let src = "flowchart LR\n  a[\"Quoted (x)\"]\n";
    let out = edit(
        src,
        Edit::SetLabel {
            id: "a".into(),
            label: "Plain".into(),
        },
    );
    assert_eq!(out, "flowchart LR\n  a[\"Plain\"]\n");
}

#[test]
fn renaming_new_style_shape_labels() {
    let src = "flowchart LR\n  s@{ shape: cyl, label: \"Event store\" }\n";
    let out = edit(
        src,
        Edit::SetLabel {
            id: "s".into(),
            label: "Outbox".into(),
        },
    );
    assert_eq!(out, "flowchart LR\n  s@{ shape: cyl, label: \"Outbox\" }\n");
}

#[test]
fn adding_nodes_and_edges_appends_lines_with_the_files_indentation() {
    let src = fixture("01-basic.mmd");
    let out = edit(
        &src,
        Edit::AddNode {
            id: "cache".into(),
            label: "Cache".into(),
            shape: Some("cylinder".into()),
        },
    );
    assert_eq!(
        out,
        "flowchart LR\n  api[Orders API] --> db[(Commerce DB)]\n  cache[(Cache)]\n"
    );
    let out = edit(
        &out,
        Edit::AddEdge {
            from: "api".into(),
            to: "cache".into(),
            label: Some("reads".into()),
        },
    );
    assert_eq!(out, "flowchart LR\n  api[Orders API] --> db[(Commerce DB)]\n  cache[(Cache)]\n  api -->|reads| cache\n");
    // Four-space files get four spaces; files without trailing newline get one.
    let out = edit(
        "graph TD\n    a --> b",
        Edit::AddEdge {
            from: "b".into(),
            to: "a".into(),
            label: None,
        },
    );
    assert_eq!(out, "graph TD\n    a --> b\n    b --> a\n");
}

#[test]
fn new_lines_go_before_the_annotation_block_and_outside_subgraphs() {
    let src = fixture("10-annotated.mmd");
    let out = edit(
        &src,
        Edit::AddNode {
            id: "web".into(),
            label: "Web".into(),
            shape: None,
        },
    );
    let lines: Vec<&str> = out.lines().collect();
    let web = lines.iter().position(|l| l.trim() == "web[Web]").unwrap();
    let first_annotation = lines
        .iter()
        .position(|l| l.contains("@schematlas"))
        .unwrap();
    assert!(web < first_annotation, "{out}");
    let src = fixture("05-subgraphs.mmd");
    let out = edit(
        &src,
        Edit::AddNode {
            id: "x".into(),
            label: "X".into(),
            shape: None,
        },
    );
    let d = mermaid::parse(&out).unwrap().diagram;
    assert!(
        d.groups.iter().all(|g| !g.nodes.contains(&"x".to_string())),
        "{out}"
    );
}

#[test]
fn removing_an_edge_keeps_its_nodes_and_their_labels() {
    let src = fixture("01-basic.mmd");
    let out = edit(&src, Edit::RemoveEdge(edge("api", "db")));
    let d = mermaid::parse(&out).unwrap().diagram;
    assert!(d.edges.is_empty());
    assert_eq!(
        d.nodes.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
        ["Orders API", "Commerce DB"]
    );
}

#[test]
fn removing_one_edge_of_a_chain_or_fanout_rewrites_only_that_statement() {
    let src = fixture("02-chains-and-fanout.mmd");
    let out = edit(&src, Edit::RemoveEdge(edge("B", "C")));
    let (removed, _) = changed_lines(&src, &out);
    assert_eq!(removed, ["    A[Start] --> B{Is it?} --> C(Done)"]);
    let d = mermaid::parse(&out).unwrap().diagram;
    assert!(!d.edges.iter().any(|e| e.from == "B" && e.to == "C"));
    assert_eq!(d.nodes.iter().find(|n| n.id == "C").unwrap().label, "Done");
    let out = edit(&src, Edit::RemoveEdge(edge("mobile", "gateway")));
    let (removed, _) = changed_lines(&src, &out);
    assert_eq!(removed, ["    web & mobile --> gateway --> users & orders"]);
}

#[test]
fn removing_a_node_removes_its_edges_styles_clicks_and_annotation() {
    let src = fixture("06-styling.mmd");
    let out = edit(&src, Edit::RemoveNode { id: "api".into() });
    assert!(!out.contains("style api"), "{out}");
    assert!(!out.contains("click api"), "{out}");
    assert!(
        out.contains("classDef hot"),
        "class definitions stay: {out}"
    );
    let d = mermaid::parse(&out).unwrap().diagram;
    assert_eq!(
        d.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["db"]
    );
    let src = fixture("11-real-world-architecture.mmd");
    let out = edit(&src, Edit::RemoveNode { id: "redis".into() });
    assert!(out.contains("class pg,s3 store"), "{out}");
    let src = fixture("10-annotated.mmd");
    let out = edit(&src, Edit::RemoveNode { id: "db".into() });
    assert!(!out.contains("@schematlas node db"), "{out}");
    assert!(!out.contains("@schematlas edge api db"), "{out}");
    assert!(out.contains("@schematlas node api"), "{out}");
}

#[test]
fn edge_labels_can_be_set_changed_and_cleared() {
    let src = fixture("04-edges.mmd");
    let out = edit(
        &src,
        Edit::SetEdgeLabel {
            edge: edge("a", "i"),
            label: Some("renamed".into()),
        },
    );
    let (removed, added) = changed_lines(&src, &out);
    assert_eq!(
        (removed, added),
        (
            vec!["  a -->|label| i".to_string()],
            vec!["  a -->|renamed| i".to_string()]
        )
    );
    let out = edit(
        &src,
        Edit::SetEdgeLabel {
            edge: edge("a", "j"),
            label: Some("new text".into()),
        },
    );
    assert!(
        out.contains("  a -- new text --> j\n") || out.contains("  a -->|new text| j\n"),
        "{out}"
    );
    let out = edit(
        &src,
        Edit::SetEdgeLabel {
            edge: edge("a", "b"),
            label: Some("added".into()),
        },
    );
    assert!(out.contains("  a -->|added| b\n"), "{out}");
    let out = edit(
        &src,
        Edit::SetEdgeLabel {
            edge: edge("a", "i"),
            label: None,
        },
    );
    assert!(out.contains("  a --> i\n"), "{out}");
    // Every stroke keeps its style through a relabel.
    // Invisible links cannot carry labels in Mermaid; the edit is refused.
    assert!(mermaid::apply(
        &src,
        &Edit::SetEdgeLabel {
            edge: edge("a", "n"),
            label: Some("x".into())
        }
    )
    .is_err());
    for to in ["c", "d", "e", "f", "g", "h", "k", "l", "m"] {
        let out = edit(
            &src,
            Edit::SetEdgeLabel {
                edge: edge("a", to),
                label: Some("x y".into()),
            },
        );
        let before = mermaid::parse(&src).unwrap().diagram;
        let after = mermaid::parse(&out).unwrap().diagram;
        let (b, a) = (
            before.edges.iter().find(|e| e.to == to).unwrap(),
            after.edges.iter().find(|e| e.to == to).unwrap(),
        );
        assert_eq!(
            (a.stroke, a.head, a.both_ends, a.length),
            (b.stroke, b.head, b.both_ends, b.length),
            "{to}: {out}"
        );
        assert_eq!(a.label.as_deref(), Some("x y"), "{to}: {out}");
    }
}

#[test]
fn annotations_are_written_updated_and_removed_in_place() {
    let src = fixture("01-basic.mmd");
    let mut note = Annotation {
        code: vec![CodeRef::parse("src/api.rs:1-9")],
        note: Some("Entry point".into()),
        ..Annotation::default()
    };
    let out = edit(
        &src,
        Edit::Annotate {
            target: Target::Node("api".into()),
            annotation: note.clone(),
        },
    );
    assert_eq!(
        out,
        "flowchart LR\n  api[Orders API] --> db[(Commerce DB)]\n\n  %% @schematlas node api code=src/api.rs:1-9 note=\"Entry point\"\n"
    );
    note.note = Some("Changed".into());
    let again = edit(
        &out,
        Edit::Annotate {
            target: Target::Node("api".into()),
            annotation: note,
        },
    );
    assert_eq!(again.matches("@schematlas node api").count(), 1);
    assert!(again.contains("note=Changed"), "{again}");
    let cleared = edit(
        &again,
        Edit::Annotate {
            target: Target::Node("api".into()),
            annotation: Annotation::default(),
        },
    );
    assert!(!cleared.contains("@schematlas"), "{cleared}");
    assert_eq!(
        mermaid::parse(&cleared).unwrap().diagram,
        mermaid::parse(&src).unwrap().diagram
    );
}

#[test]
fn edits_that_make_no_sense_are_errors_and_change_nothing() {
    let src = fixture("01-basic.mmd");
    for bad in [
        Edit::SetLabel {
            id: "nope".into(),
            label: "x".into(),
        },
        Edit::RemoveNode { id: "nope".into() },
        Edit::AddNode {
            id: "api".into(),
            label: "dup".into(),
            shape: None,
        },
        Edit::AddNode {
            id: "has space".into(),
            label: "x".into(),
            shape: None,
        },
        Edit::AddNode {
            id: "end".into(),
            label: "keyword".into(),
            shape: None,
        },
        Edit::AddEdge {
            from: "api".into(),
            to: "ghost".into(),
            label: None,
        },
        Edit::RemoveEdge(edge("db", "api")),
        Edit::SetLabel {
            id: "api".into(),
            label: "".into(),
        },
    ] {
        assert!(mermaid::apply(&src, &bad).is_err(), "{bad:?}");
    }
}

#[test]
fn frontmatter_init_comments_and_accessibility_survive_every_edit() {
    let src = fixture("08-frontmatter-and-init.mmd");
    let out = edit(
        &src,
        Edit::AddEdge {
            from: "psp".into(),
            to: "checkout".into(),
            label: Some("webhook".into()),
        },
    );
    assert!(
        out.starts_with("---\ntitle: Payments flow\nconfig:\n  theme: dark\n---\n%%{init:"),
        "{out}"
    );
    assert!(
        out.contains("  accDescr {\n    How a card payment travels\n"),
        "{out}"
    );
    let src = fixture("07-comments-and-semicolons.mmd");
    let out = edit(
        &src,
        Edit::SetLabel {
            id: "server".into(),
            label: "API".into(),
        },
    );
    assert!(out.contains("%% the entry point"));
    assert!(
        out.contains("client[Client]; client --> server[API]"),
        "{out}"
    );
}

#[test]
fn crlf_files_stay_crlf_and_tabs_stay_tabs() {
    let src = fixture("14-crlf.mmd");
    let out = edit(
        &src,
        Edit::AddEdge {
            from: "c".into(),
            to: "a".into(),
            label: None,
        },
    );
    assert!(out.contains("  b --> c\r\n  c --> a\r\n"), "{out:?}");
    assert_eq!(
        out.matches('\n').count(),
        out.matches("\r\n").count(),
        "{out:?}"
    );
    let src = fixture("15-tabs.mmd");
    let out = edit(
        &src,
        Edit::AddNode {
            id: "w".into(),
            label: "W".into(),
            shape: None,
        },
    );
    assert!(out.ends_with("\tw[W]\n"), "{out:?}");
}

#[test]
fn links_to_subgraphs_are_edges_to_groups_not_extra_nodes() {
    let src = fixture("16-edges-to-subgraphs.mmd");
    let d = mermaid::parse(&src).unwrap().diagram;
    assert!(!d.nodes.iter().any(|n| n.id == "api"));
    assert!(d.edges.iter().any(|e| e.from == "client" && e.to == "api"));
    let out = edit(
        &src,
        Edit::AddEdge {
            from: "db".into(),
            to: "api".into(),
            label: Some("replies".into()),
        },
    );
    assert!(out.contains("  db -->|replies| api\n"), "{out}");
    let out = edit(&src, Edit::RemoveEdge(edge("client", "api")));
    assert!(!mermaid::parse(&out)
        .unwrap()
        .diagram
        .edges
        .iter()
        .any(|e| e.from == "client"));
}

#[test]
fn redefined_labels_change_where_they_take_effect() {
    let src = fixture("17-redefinition-and-numbers.mmd");
    let out = edit(
        &src,
        Edit::SetLabel {
            id: "a".into(),
            label: "Three".into(),
        },
    );
    assert!(
        out.contains("  a[One] --> b\n") && out.contains("  a[Three]\n"),
        "{out}"
    );
    // Fan-out edges sharing one label are split only when one of them changes.
    let out = edit(
        &src,
        Edit::SetEdgeLabel {
            edge: edge("B", "C"),
            label: Some("own".into()),
        },
    );
    let d = mermaid::parse(&out).unwrap().diagram;
    let label = |from: &str| {
        d.edges
            .iter()
            .find(|e| e.from == from && e.to == "C")
            .unwrap()
            .label
            .clone()
    };
    assert_eq!(
        (label("A").as_deref(), label("B").as_deref()),
        (Some("shared"), Some("own"))
    );
}
