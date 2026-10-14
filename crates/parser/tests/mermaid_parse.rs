//! What the Mermaid reader understands, fixture by fixture.
mod support;
use schematlas_parser::mermaid;
use schematlas_parser::model::{Head, Stroke};
use support::fixture;

fn ids(d: &schematlas_parser::model::Diagram) -> Vec<&str> {
    d.nodes.iter().map(|n| n.id.as_str()).collect()
}
fn edges(d: &schematlas_parser::model::Diagram) -> Vec<(String, String)> {
    d.edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone()))
        .collect()
}

#[test]
fn basic_nodes_labels_shapes_and_direction() {
    let d = mermaid::parse(&fixture("01-basic.mmd")).unwrap().diagram;
    assert_eq!(d.kind, "flowchart");
    assert_eq!(d.direction.as_deref(), Some("LR"));
    assert_eq!(ids(&d), ["api", "db"]);
    assert_eq!(d.nodes[0].label, "Orders API");
    assert_eq!(d.nodes[1].label, "Commerce DB");
    assert_eq!(d.nodes[1].shape.as_deref(), Some("cylinder"));
    assert_eq!(edges(&d), [("api".into(), "db".into())]);
    assert_eq!(d.edges[0].label, None);
}

#[test]
fn chains_and_fanout_expand_to_every_edge_in_order() {
    let d = mermaid::parse(&fixture("02-chains-and-fanout.mmd"))
        .unwrap()
        .diagram;
    // Mermaid treats TD and TB as the same direction; the model uses TB.
    assert_eq!(d.direction.as_deref(), Some("TB"));
    let pairs: Vec<String> = d
        .edges
        .iter()
        .map(|e| format!("{}>{}", e.from, e.to))
        .collect();
    assert_eq!(
        pairs,
        [
            "A>B",
            "B>C",
            "B>D",
            "web>gateway",
            "mobile>gateway",
            "gateway>users",
            "gateway>orders"
        ]
    );
    assert_eq!(d.edges[2].label.as_deref(), Some("No"));
    assert_eq!(
        d.nodes
            .iter()
            .find(|n| n.id == "B")
            .unwrap()
            .shape
            .as_deref(),
        Some("diamond")
    );
}

#[test]
fn every_classic_shape_is_recognised() {
    let d = mermaid::parse(&fixture("03-shapes.mmd")).unwrap().diagram;
    assert_eq!(d.nodes.len(), 16);
    let shape = |id: &str| {
        d.nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .shape
            .clone()
            .unwrap_or_default()
    };
    assert_eq!(shape("cyl"), "cylinder");
    assert_eq!(shape("newcyl"), "cylinder");
    assert_eq!(shape("rh"), "diamond");
    assert_eq!(shape("ci"), "circle");
    assert_eq!(
        d.nodes.iter().find(|n| n.id == "newcyl").unwrap().label,
        "Event store"
    );
    // A bare id is a node labelled with its id.
    assert_eq!(
        d.nodes.iter().find(|n| n.id == "bare").unwrap().label,
        "bare"
    );
}

#[test]
fn edge_strokes_heads_lengths_and_label_forms() {
    let d = mermaid::parse(&fixture("04-edges.mmd")).unwrap().diagram;
    let e = |to: &str| d.edges.iter().find(|e| e.to == to).unwrap().clone();
    assert_eq!((e("b").stroke, e("b").head), (Stroke::Normal, Head::Arrow));
    assert_eq!(e("c").head, Head::None);
    assert_eq!(e("d").stroke, Stroke::Dotted);
    assert_eq!(e("e").stroke, Stroke::Thick);
    assert_eq!(e("f").head, Head::Circle);
    assert_eq!(e("g").head, Head::Cross);
    assert!(e("h").both_ends);
    assert_eq!(e("i").label.as_deref(), Some("label"));
    assert_eq!(e("j").label.as_deref(), Some("inline text"));
    assert_eq!(
        (e("k").label.as_deref(), e("k").stroke),
        (Some("dotted text"), Stroke::Dotted)
    );
    assert_eq!(
        (e("l").label.as_deref(), e("l").stroke),
        (Some("thick text"), Stroke::Thick)
    );
    assert_eq!(e("m").length, 3);
    assert_eq!(e("n").stroke, Stroke::Invisible);
}

#[test]
fn subgraphs_become_nested_groups() {
    let d = mermaid::parse(&fixture("05-subgraphs.mmd"))
        .unwrap()
        .diagram;
    let group = |id: &str| d.groups.iter().find(|g| g.id == id).unwrap().clone();
    assert_eq!(group("backend").title, "Back end");
    assert_eq!(group("backend").direction.as_deref(), Some("TB"));
    assert_eq!(group("jobs").parent.as_deref(), Some("backend"));
    assert_eq!(group("jobs").nodes, ["worker", "queue"]);
    assert!(group("backend").nodes.contains(&"api".to_string()));
    assert_eq!(group("Frontend").title, "Frontend");
    assert_eq!(group("Frontend").nodes, ["web"]);
}

#[test]
fn styling_links_and_classes_are_read_not_lost() {
    let d = mermaid::parse(&fixture("06-styling.mmd")).unwrap().diagram;
    let api = d.nodes.iter().find(|n| n.id == "api").unwrap();
    assert!(api.classes.contains(&"hot".to_string()));
    assert_eq!(api.link.as_deref(), Some("https://example.com/api"));
    let db = d.nodes.iter().find(|n| n.id == "db").unwrap();
    assert!(db.classes.contains(&"cold".to_string()));
}

#[test]
fn comments_semicolons_frontmatter_and_accessibility() {
    let d = mermaid::parse(&fixture("07-comments-and-semicolons.mmd"))
        .unwrap()
        .diagram;
    assert_eq!(ids(&d), ["client", "server", "cache"]);
    let d = mermaid::parse(&fixture("08-frontmatter-and-init.mmd"))
        .unwrap()
        .diagram;
    assert_eq!(d.title.as_deref(), Some("Payments flow"));
    assert_eq!(d.edges[0].label.as_deref(), Some("charge"));
    assert_eq!(ids(&d), ["checkout", "psp"]);
}

#[test]
fn quoted_unicode_and_entity_labels_are_decoded() {
    let d = mermaid::parse(&fixture("09-quotes-unicode-entities.mmd"))
        .unwrap()
        .diagram;
    let label = |id: &str| d.nodes.iter().find(|n| n.id == id).unwrap().label.clone();
    assert_eq!(label("a"), "Label with (parens) and [brackets]");
    assert_eq!(label("b"), "Ünïcödé — 日本語 🚀");
    assert_eq!(label("c"), "Quote \"inside\" and # hash");
    assert!(label("d").contains("**Markdown** string"));
    assert_eq!(
        d.edges.last().unwrap().label.as_deref(),
        Some("edge with | pipe")
    );
}

#[test]
fn ids_with_dashes_dots_and_edge_ids() {
    let d = mermaid::parse(&fixture("12-ids-and-keywords.mmd"))
        .unwrap()
        .diagram;
    assert_eq!(
        ids(&d),
        [
            "node_1",
            "node-2",
            "user.service",
            "end_node",
            "A1",
            "A2",
            "A3"
        ]
    );
    assert_eq!(d.edges.last().unwrap().id.as_deref(), Some("e1"));
}

#[test]
fn empty_and_invalid_sources() {
    let d = mermaid::parse(&fixture("13-empty.mmd")).unwrap().diagram;
    assert!(d.nodes.is_empty() && d.edges.is_empty());
    assert!(mermaid::parse("").is_err());
    assert!(mermaid::parse("not a diagram at all").is_err());
    let error = mermaid::parse("flowchart LR\n  a --> \n  b[unclosed").unwrap_err();
    assert!(!error.to_string().is_empty());
    // Other diagram types are recognised but not editable as graphs yet.
    let error = mermaid::parse("sequenceDiagram\n  Alice->>Bob: hi").unwrap_err();
    assert!(error.to_string().contains("sequence"), "{error}");
}
