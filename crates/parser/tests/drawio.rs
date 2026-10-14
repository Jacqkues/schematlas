//! draw.io files: pages, containers, HTML labels, compression, custom properties as annotations,
//! and edits that change one element at a time.
use schematlas_parser::annotate::Target;
use schematlas_parser::drawio;
use schematlas_parser::model::{self, Annotation, CodeRef, EdgeRef, Edit, Head, Stroke};
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/drawio")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}
fn page(name: &str, index: usize) -> model::Diagram {
    drawio::parse(&fixture(name)).unwrap().remove(index).diagram
}
fn edge(from: &str, to: &str) -> EdgeRef {
    EdgeRef {
        from: from.into(),
        to: to.into(),
        nth: 0,
    }
}

#[test]
fn reads_cells_html_labels_shapes_and_absolute_positions() {
    let pages = drawio::parse(&fixture("01-basic.drawio")).unwrap();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].name, "Checkout");
    let d = &pages[0].diagram;
    assert_eq!(d.kind, "drawio");
    assert_eq!(
        d.nodes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
        ["api", "db"]
    );
    assert_eq!(d.nodes[0].label, "Orders API", "<br> becomes a space");
    assert_eq!(d.nodes[0].shape.as_deref(), Some("rounded"));
    assert_eq!(d.nodes[1].shape.as_deref(), Some("cylinder"));
    assert_eq!(d.edges[0].label.as_deref(), Some("writes"));
    assert_eq!(d.edges[0].id.as_deref(), Some("e1"));
    let api = d.layout["api"];
    assert_eq!(
        (api.x, api.y, api.width, api.height),
        (80.0, 120.0, 120.0, 60.0)
    );
}

#[test]
fn containers_become_groups_with_absolute_child_positions() {
    let d = page("03-containers.drawio", 0);
    let group = |id: &str| d.groups.iter().find(|g| g.id == id).unwrap().clone();
    assert_eq!(group("backend").title, "Back end");
    assert_eq!(group("backend").nodes, ["svc"]);
    assert_eq!(group("jobs").parent.as_deref(), Some("backend"));
    assert_eq!(group("jobs").nodes, ["worker"]);
    // Children are stored relative to their container; the model is absolute.
    let worker = d.layout["worker"];
    assert_eq!(
        (worker.x, worker.y),
        (200.0 + 180.0 + 40.0, 40.0 + 120.0 + 50.0)
    );
    assert_eq!(
        d.nodes.iter().find(|n| n.id == "svc").unwrap().label,
        "Order service"
    );
    // An edge label stored as a child cell is the edge's label, not a node.
    assert!(!d.nodes.iter().any(|n| n.id == "call-label"));
    let call = d
        .edges
        .iter()
        .find(|e| e.id.as_deref() == Some("call"))
        .unwrap();
    assert_eq!(call.label.as_deref(), Some("HTTPS"));
    assert_eq!(
        (call.stroke, call.head, call.both_ends),
        (Stroke::Dotted, Head::Circle, true)
    );
    let enqueue = d
        .edges
        .iter()
        .find(|e| e.id.as_deref() == Some("enqueue"))
        .unwrap();
    assert_eq!((enqueue.stroke, enqueue.head), (Stroke::Thick, Head::None));
    // A connector without a target is drawn but is not an edge of the graph.
    assert!(!d.edges.iter().any(|e| e.id.as_deref() == Some("dangling")));
    assert_eq!(
        d.nodes
            .iter()
            .find(|n| n.id == "note")
            .unwrap()
            .shape
            .as_deref(),
        Some("text")
    );
}

#[test]
fn compressed_pages_read_like_plain_ones() {
    let plain = page("01-basic.drawio", 0);
    let compressed = drawio::parse(&fixture("02-compressed.drawio")).unwrap();
    assert_eq!(compressed.len(), 2);
    assert_eq!(compressed[0].diagram, plain);
    assert_eq!(compressed[1].diagram, page("03-containers.drawio", 0));
    let mixed = drawio::parse(&fixture("05-multipage.drawio")).unwrap();
    assert_eq!(
        mixed.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        ["Overview", "Details"]
    );
}

#[test]
fn custom_properties_are_annotations_and_others_survive() {
    let d = page("04-annotated.drawio", 0);
    assert_eq!(d.annotation.title.as_deref(), Some("Checkout architecture"));
    assert_eq!(d.annotation.kind.as_deref(), Some("architecture"));
    let api = d.nodes.iter().find(|n| n.id == "api").unwrap();
    assert_eq!(api.label, "Orders API");
    assert_eq!(
        api.annotation.code,
        [
            CodeRef::parse("src/api/checkout.rs:18-64"),
            CodeRef::parse("src/api/mod.rs")
        ]
    );
    assert_eq!(api.annotation.note.as_deref(), Some("Validates carts."));
    assert_eq!(api.annotation.tags, ["http", "core"]);
    assert_eq!(
        api.annotation.extra.get("future").map(String::as_str),
        Some("kept")
    );
    assert_eq!(
        d.nodes
            .iter()
            .find(|n| n.id == "ledger")
            .unwrap()
            .annotation
            .kind
            .as_deref(),
        Some("store")
    );
    let post = &d.edges[0];
    assert_eq!(
        (post.label.as_deref(), post.annotation.code.len()),
        (Some("posts"), 1)
    );
    // Annotating keeps the user's own properties.
    let a = Annotation {
        note: Some("Changed".into()),
        ..Annotation::default()
    };
    let out = drawio::apply(
        &fixture("04-annotated.drawio"),
        0,
        &Edit::Annotate {
            target: Target::Node("api".into()),
            annotation: a,
        },
    )
    .unwrap();
    let api_line = out.lines().find(|l| l.contains("id=\"api\"")).unwrap();
    assert!(api_line.contains("owner=\"payments-team\""), "{api_line}");
    assert!(
        api_line.contains("schematlas-note=\"Changed\""),
        "{api_line}"
    );
    assert!(!api_line.contains("schematlas-code"), "{api_line}");
}

#[test]
fn exported_svgs_are_readable_but_not_editable() {
    let pages = drawio::parse(&fixture("07-exported.drawio.svg")).unwrap();
    assert_eq!(pages[0].diagram, page("01-basic.drawio", 0));
    let err = drawio::apply(
        &fixture("07-exported.drawio.svg"),
        0,
        &Edit::RemoveNode { id: "api".into() },
    )
    .unwrap_err();
    assert!(err.to_string().contains(".drawio"), "{err}");
}

fn changed_lines(before: &str, after: &str) -> usize {
    let a: Vec<&str> = before.lines().collect();
    after.lines().filter(|l| !a.contains(l)).count()
}

#[test]
fn edits_touch_one_element_at_a_time() {
    let src = fixture("01-basic.drawio");
    let out = drawio::apply(
        &src,
        0,
        &Edit::SetLabel {
            id: "db".into(),
            label: "Ledger <primary>".into(),
        },
    )
    .unwrap();
    assert_eq!(changed_lines(&src, &out), 1, "{out}");
    assert!(
        out.contains("value=\"Ledger &amp;lt;primary&amp;gt;\""),
        "HTML labels are escaped twice: {out}"
    );
    assert_eq!(
        drawio::parse(&out).unwrap()[0].diagram.nodes[1].label,
        "Ledger <primary>"
    );
    let out = drawio::apply(
        &src,
        0,
        &Edit::Move {
            id: "api".into(),
            x: 10.0,
            y: 20.0,
        },
    )
    .unwrap();
    assert_eq!(changed_lines(&src, &out), 1, "{out}");
    let out = drawio::apply(
        &src,
        0,
        &Edit::SetEdgeLabel {
            edge: edge("api", "db"),
            label: None,
        },
    )
    .unwrap();
    assert_eq!(changed_lines(&src, &out), 1, "{out}");
    let out = drawio::apply(&src, 0, &Edit::RemoveNode { id: "db".into() }).unwrap();
    let d = drawio::parse(&out).unwrap().remove(0).diagram;
    assert_eq!((d.nodes.len(), d.edges.len()), (1, 0));
    assert!(
        out.contains("<mxGraphModel dx=\"1042\""),
        "untouched elements keep their text: {out}"
    );
}

#[test]
fn new_nodes_get_a_free_spot_and_new_edges_connect() {
    let src = fixture("01-basic.drawio");
    let out = drawio::apply(
        &src,
        0,
        &Edit::AddNode {
            id: "cache".into(),
            label: "Cache".into(),
            shape: Some("cylinder".into()),
        },
    )
    .unwrap();
    let d = drawio::parse(&out).unwrap().remove(0).diagram;
    let cache = d.layout["cache"];
    for (id, other) in &d.layout {
        if id != "cache" {
            let apart = cache.x >= other.x + other.width
                || cache.x + cache.width <= other.x
                || cache.y >= other.y + other.height
                || cache.y + cache.height <= other.y;
            assert!(apart, "cache overlaps {id}");
        }
    }
    assert_eq!(d.nodes.last().unwrap().shape.as_deref(), Some("cylinder"));
    let out = drawio::apply(
        &out,
        0,
        &Edit::AddEdge {
            from: "api".into(),
            to: "cache".into(),
            label: Some("reads".into()),
        },
    )
    .unwrap();
    let d = drawio::parse(&out).unwrap().remove(0).diagram;
    assert!(d
        .edges
        .iter()
        .any(|e| e.from == "api" && e.to == "cache" && e.label.as_deref() == Some("reads")));
}

#[test]
fn compressed_pages_stay_compressed_and_other_pages_untouched() {
    let src = fixture("05-multipage.drawio");
    let out = drawio::apply(
        &src,
        1,
        &Edit::SetLabel {
            id: "worker".into(),
            label: "Job runner".into(),
        },
    )
    .unwrap();
    let first_page_end = src.find("</diagram>").unwrap();
    assert_eq!(&out[..first_page_end], &src[..first_page_end]);
    assert!(
        !out.contains("Job runner"),
        "the edited page is still compressed"
    );
    let pages = drawio::parse(&out).unwrap();
    assert_eq!(
        pages[1]
            .diagram
            .nodes
            .iter()
            .find(|n| n.id == "worker")
            .unwrap()
            .label,
        "Job runner"
    );
    assert_eq!(pages[0].diagram, drawio::parse(&src).unwrap()[0].diagram);
}

#[test]
fn every_edit_agrees_with_the_model_on_every_page() {
    let mut checked = 0;
    for name in [
        "01-basic.drawio",
        "02-compressed.drawio",
        "03-containers.drawio",
        "04-annotated.drawio",
        "05-multipage.drawio",
        "06-shapes.drawio",
    ] {
        let src = fixture(name);
        for (index, page) in drawio::parse(&src).unwrap().into_iter().enumerate() {
            let d = page.diagram;
            let mut edits = vec![Edit::AddNode {
                id: "fresh".into(),
                label: "Fresh & new".into(),
                shape: Some("diamond".into()),
            }];
            for n in &d.nodes {
                edits.push(Edit::SetLabel {
                    id: n.id.clone(),
                    label: format!("{} <2>", n.label),
                });
                edits.push(Edit::RemoveNode { id: n.id.clone() });
                edits.push(Edit::Move {
                    id: n.id.clone(),
                    x: 5.0,
                    y: 7.5,
                });
                edits.push(Edit::Annotate {
                    target: Target::Node(n.id.clone()),
                    annotation: Annotation {
                        code: vec![CodeRef::parse("src/x.rs:1")],
                        note: Some("n\"ote".into()),
                        ..Annotation::default()
                    },
                });
            }
            for (i, _) in d.edges.iter().enumerate() {
                let r = EdgeRef::of(&d, i);
                edits.push(Edit::RemoveEdge(r.clone()));
                edits.push(Edit::SetEdgeLabel {
                    edge: r.clone(),
                    label: Some("re-labelled".into()),
                });
                edits.push(Edit::SetEdgeLabel {
                    edge: r,
                    label: None,
                });
            }
            if d.nodes.len() >= 2 {
                edits.push(Edit::AddEdge {
                    from: d.nodes[1].id.clone(),
                    to: d.nodes[0].id.clone(),
                    label: None,
                });
            }
            edits.push(Edit::Annotate {
                target: Target::Diagram,
                annotation: Annotation {
                    title: Some("T".into()),
                    ..Annotation::default()
                },
            });
            for edit in edits {
                let Ok(expected) = model::apply(&d, &edit) else {
                    assert!(
                        drawio::apply(&src, index, &edit).is_err(),
                        "{name}#{index}: {edit:?}"
                    );
                    continue;
                };
                let out = drawio::apply(&src, index, &edit)
                    .unwrap_or_else(|e| panic!("{name}#{index}: {edit:?}: {e}"));
                let after = drawio::parse(&out)
                    .unwrap_or_else(|e| panic!("{name}#{index}: {edit:?}: {e}\n{out}"));
                assert!(
                    after[index].diagram.same_as(&expected),
                    "{name}#{index}: {edit:?}\n{out}\n{:#?}\n{:#?}",
                    after[index].diagram,
                    expected
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 150, "{checked}");
}

#[test]
fn not_drawio() {
    assert!(drawio::parse("").is_err());
    assert!(drawio::parse("<svg></svg>").is_err());
    assert!(drawio::parse("<mxfile><diagram name=\"x\">not base64!</diagram></mxfile>").is_err());
}

#[test]
fn snapshots() {
    insta::glob!("fixtures/drawio/*", |path| {
        let src = std::fs::read_to_string(path).unwrap();
        let pages = drawio::parse(&src).unwrap();
        insta::assert_yaml_snapshot!(pages
            .iter()
            .map(|p| (&p.name, &p.diagram))
            .collect::<Vec<_>>());
    });
}
