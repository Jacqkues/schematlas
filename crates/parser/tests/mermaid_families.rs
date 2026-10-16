//! Class, entity-relationship, state and sequence diagrams read into the same graph model.
mod support;
use schematlas_parser::annotate::Target;
use schematlas_parser::mermaid;
use schematlas_parser::model::{
    Annotation, Arrow, CodeRef, Edit, Head, NotePlacement, Step, Stroke,
};
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
    assert!(mermaid::parse("gantt\n  title Plan")
        .unwrap_err()
        .to_string()
        .contains("gantt"));
}

const SEQUENCE: &str = "sequenceDiagram
  autonumber
  actor U as User
  participant W as Web<br>app
  participant A as API
  participant D@{ \"type\": \"database\" }
  U->>W: Checkout
  activate W
  W->>+A: POST /orders
  Note over W,A: JSON body
  loop Every item
    A->>D: reserve stock
  end
  alt paid
    A-->>W: 201 Created
  else declined
    A--xW: 402
  end
  A-)D: audit
  Note right of D: async
  deactivate W
  A->>A: self call
";

#[test]
fn sequence_diagrams_keep_participants_and_every_step_in_order() {
    let d = mermaid::parse(SEQUENCE).unwrap().diagram;
    assert_eq!(d.kind, "sequence");
    let participants: Vec<(&str, &str, Option<&str>)> = d
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.label.as_str(), n.shape.as_deref()))
        .collect();
    assert_eq!(
        participants,
        [
            ("U", "User", Some("actor")),
            ("W", "Web<br>app", None),
            ("A", "API", None),
            ("D", "D", Some("cylinder")),
        ]
    );
    assert_eq!(d.nodes[0].kind(), "actor");
    assert_eq!(d.nodes[3].kind(), "store");
    let seq = d.sequence.as_ref().unwrap();
    assert!(seq.autonumber);
    let message = |from: &str, to: &str, label: &str, dotted: bool, arrow: Arrow| Step::Message {
        from: from.into(),
        to: to.into(),
        label: label.into(),
        dotted,
        arrow,
    };
    assert_eq!(
        seq.steps,
        [
            message("U", "W", "Checkout", false, Arrow::Filled),
            Step::Activate { actor: "W".into() },
            message("W", "A", "POST /orders", false, Arrow::Filled),
            Step::Activate { actor: "A".into() },
            Step::Note {
                over: vec!["W".into(), "A".into()],
                placement: NotePlacement::Over,
                text: "JSON body".into(),
            },
            Step::Start {
                block: "loop".into(),
                label: "Every item".into(),
            },
            message("A", "D", "reserve stock", false, Arrow::Filled),
            Step::End,
            Step::Start {
                block: "alt".into(),
                label: "paid".into(),
            },
            message("A", "W", "201 Created", true, Arrow::Filled),
            Step::Branch {
                label: "declined".into(),
            },
            message("A", "W", "402", true, Arrow::Cross),
            Step::End,
            message("A", "D", "audit", false, Arrow::Async),
            Step::Note {
                over: vec!["D".into()],
                placement: NotePlacement::RightOf,
                text: "async".into(),
            },
            Step::Deactivate { actor: "W".into() },
            message("A", "A", "self call", false, Arrow::Filled),
        ]
    );
    // Messages are edges too, so connections and edge annotations work as elsewhere.
    assert_eq!(d.edges.len(), 7);
    assert_eq!(d.edges[3].stroke, Stroke::Dotted);
    assert_eq!(d.edges[3].label.as_deref(), Some("201 Created"));
}

#[test]
fn sequence_participants_take_annotations_but_not_structural_edits() {
    let note = Annotation {
        code: vec![CodeRef::parse("src/api/checkout.rs:18-64")],
        note: Some("Validates the basket".into()),
        ..Annotation::default()
    };
    let out = mermaid::apply(
        SEQUENCE,
        &Edit::Annotate {
            target: Target::Node("A".into()),
            annotation: note.clone(),
        },
    )
    .unwrap();
    assert!(out.starts_with(SEQUENCE.trim_end()), "{out}");
    let d = mermaid::parse(&out).unwrap().diagram;
    assert_eq!(d.nodes[2].annotation, note);
    assert_eq!(d.sequence.as_ref().unwrap().steps.len(), 17);
    let err = mermaid::apply(SEQUENCE, &Edit::RemoveNode { id: "A".into() }).unwrap_err();
    assert!(err.to_string().contains("sequence"), "{err}");
}
