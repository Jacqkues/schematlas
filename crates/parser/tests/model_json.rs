//! The model travels as JSON between the desktop backend and the UI, which reads it with the
//! same types: everything left out when empty must read back as empty.
use schematlas_parser::annotate::Target;
use schematlas_parser::mermaid;
use schematlas_parser::model::{Diagram, Edit};

#[test]
fn diagrams_and_edits_round_trip_through_json() {
    let src = "flowchart LR\n  a[A] -->|calls| b[(B)]\n  subgraph g[Group]\n    b\n  end\n%% @schematlas node a code=src/a.rs:3 note=\"Entry\"\n";
    let diagram = mermaid::parse(src).unwrap().diagram;
    let json = serde_json::to_string(&diagram).unwrap();
    assert!(!json.contains("\"layout\""), "{json}");
    let back: Diagram = serde_json::from_str(&json).unwrap();
    assert_eq!(back, diagram);
    let bare: Diagram = serde_json::from_str(
        r#"{"kind":"flowchart","nodes":[{"id":"x","label":"X"}],"edges":[{"from":"x","to":"x"}]}"#,
    )
    .unwrap();
    assert_eq!(bare.edges[0].length, 1);
    let edits = vec![
        Edit::Annotate {
            target: Target::Node("a".into()),
            annotation: diagram.nodes[0].annotation.clone(),
        },
        Edit::Move {
            id: "a".into(),
            x: 1.0,
            y: 2.0,
        },
    ];
    let back: Vec<Edit> = serde_json::from_str(&serde_json::to_string(&edits).unwrap()).unwrap();
    assert_eq!(back, edits);
}
