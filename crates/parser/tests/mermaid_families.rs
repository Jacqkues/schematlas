//! Class, entity-relationship and state diagrams read into the same graph model.
mod support;
use schematlas_parser::annotate::Target;
use schematlas_parser::mermaid;
use schematlas_parser::model::{Annotation, CodeRef, Edit, Head, Stroke};
use support::fixture;

#[test]
fn class_diagrams_have_members_and_uml_relationships() {
    let d = mermaid::parse(&fixture("20-class.mmd")).unwrap().diagram;
    assert_eq!(d.kind, "class");
    assert_eq!(d.direction.as_deref(), Some("LR"));
    let order = d.nodes.iter().find(|n| n.id == "Order").unwrap();
    assert_eq!(
        order.details,
        [
            "+String id",
            "+Money total",
            "+place() : void",
            "-validate() : bool"
        ]
    );
    assert_eq!(
        order.annotation.code,
        [CodeRef::parse("src/orders/order.rs:1-120")]
    );
    // Classes only named in relationships are nodes too.
    assert!(d.nodes.iter().any(|n| n.id == "PaymentGateway"));
    let edge = |a: &str, b: &str| {
        d.edges
            .iter()
            .find(|e| e.from == a && e.to == b)
            .unwrap_or_else(|| panic!("{a} -> {b}: {:#?}", d.edges))
            .clone()
    };
    let places = edge("Customer", "Order");
    assert_eq!(places.label.as_deref(), Some("places"));
    assert_eq!(
        (places.tail_label.as_deref(), places.head_label.as_deref()),
        (Some("1"), Some("*"))
    );
    // Inheritance points at the parent with a triangle; composition is a filled diamond.
    assert_eq!(edge("Shipment", "Event").head, Head::Triangle);
    assert_eq!(edge("LineItem", "Order").head, Head::FilledDiamond);
    assert_eq!(edge("LineItem", "Cart").head, Head::Diamond);
    let dependency = edge("Order", "PaymentGateway");
    assert_eq!(
        (dependency.stroke, dependency.head),
        (Stroke::Dotted, Head::Arrow)
    );
}

#[test]
fn er_diagrams_have_attributes_and_cardinalities() {
    let d = mermaid::parse(&fixture("21-er.mmd")).unwrap().diagram;
    assert_eq!(d.kind, "er");
    let customer = d.nodes.iter().find(|n| n.id == "CUSTOMER").unwrap();
    assert_eq!(
        customer.details,
        ["string id PK", "string email UK \"login identifier\""]
    );
    assert_eq!(customer.shape.as_deref(), Some("entity"));
    let places = d
        .edges
        .iter()
        .find(|e| e.label.as_deref() == Some("places"))
        .unwrap();
    assert_eq!(
        (places.from.as_str(), places.to.as_str()),
        ("CUSTOMER", "ORDER")
    );
    assert_eq!(
        (places.tail_label.as_deref(), places.head_label.as_deref()),
        (Some("1"), Some("0..n"))
    );
    let appears = d
        .edges
        .iter()
        .find(|e| e.label.as_deref() == Some("appears in"))
        .unwrap();
    assert_eq!(
        appears.stroke,
        Stroke::Dotted,
        "non-identifying relationships are dashed"
    );
    assert_eq!(
        d.nodes
            .iter()
            .find(|n| n.id == "ORDER")
            .unwrap()
            .annotation
            .code
            .len(),
        1
    );
}

#[test]
fn state_diagrams_have_start_end_and_composite_states() {
    let d = mermaid::parse(&fixture("22-state.mmd")).unwrap().diagram;
    assert_eq!(d.kind, "state");
    let shapes: Vec<(&str, Option<&str>)> = d
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.shape.as_deref()))
        .collect();
    assert!(
        shapes.contains(&("root_start", Some("start"))),
        "{shapes:?}"
    );
    assert!(shapes.contains(&("root_end", Some("end"))), "{shapes:?}");
    let approved = d.groups.iter().find(|g| g.id == "Approved").unwrap();
    assert!(approved.nodes.contains(&"Published".to_string()));
    assert!(d.edges.iter().any(|e| e.from == "Submitted"
        && e.to == "Approved"
        && e.label.as_deref() == Some("approve")));
    // A transition into a composite state targets the group.
    assert!(d.edges.iter().any(|e| e.to == "Approved"));
}

#[test]
fn other_families_take_annotations_but_not_structural_edits() {
    let src = fixture("20-class.mmd");
    let note = Annotation {
        note: Some("Aggregate root".into()),
        ..Annotation::default()
    };
    let out = mermaid::apply(
        &src,
        &Edit::Annotate {
            target: Target::Node("Customer".into()),
            annotation: note.clone(),
        },
    )
    .unwrap();
    let d = mermaid::parse(&out).unwrap().diagram;
    assert_eq!(
        d.nodes
            .iter()
            .find(|n| n.id == "Customer")
            .unwrap()
            .annotation,
        note
    );
    let err = mermaid::apply(
        &src,
        &Edit::RemoveNode {
            id: "Customer".into(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("class"), "{err}");
    // Families with no graph shape are still refused with their name.
    assert!(mermaid::parse("sequenceDiagram\n  A->>B: hi")
        .unwrap_err()
        .to_string()
        .contains("sequence"));
}
