//! Randomly generated draw.io pages, edited many times in a row: every intermediate file must
//! read back as the model says, including after wrapping cells in UserObjects and moving
//! containers.
mod support;
use schematlas_parser::annotate::Target;
use schematlas_parser::drawio;
use schematlas_parser::model::{self, Annotation, CodeRef, EdgeRef, Edit};
use support::Rng;

const STYLES: [&str; 6] = [
    "rounded=0;whiteSpace=wrap;html=1;",
    "rounded=1;whiteSpace=wrap;html=1;",
    "shape=cylinder3;whiteSpace=wrap;html=1;",
    "rhombus;whiteSpace=wrap;html=1;",
    "ellipse;whiteSpace=wrap;html=1;",
    "whiteSpace=wrap;",
];
const LABELS: [&str; 5] = [
    "Orders API",
    "Cache &amp;lt;hot&amp;gt;",
    "Line&lt;br&gt;break",
    "Ünïcode",
    "a &amp;amp; b",
];

fn generate(seed: u64, compressed: bool) -> String {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut cells = String::new();
    let mut vertices: Vec<String> = vec![];
    let mut containers: Vec<String> = vec!["1".into()];
    for i in 0..3 + rng.below(8) {
        let parent = containers[rng.below(containers.len())].clone();
        let id = format!("v{i}");
        let (x, y) = (rng.below(400) * 2, rng.below(300) * 2);
        if rng.below(5) == 0 {
            cells.push_str(&format!(
                "        <mxCell id=\"{id}\" value=\"Box {i}\" style=\"swimlane;whiteSpace=wrap;html=1;\" vertex=\"1\" parent=\"{parent}\">\n          <mxGeometry x=\"{x}\" y=\"{y}\" width=\"300\" height=\"200\" as=\"geometry\" />\n        </mxCell>\n"
            ));
            containers.push(id);
            continue;
        }
        let style = STYLES[rng.below(STYLES.len())];
        let label = LABELS[rng.below(LABELS.len())];
        if rng.below(3) == 0 {
            cells.push_str(&format!(
                "        <UserObject label=\"{label}\" owner=\"team-{i}\" schematlas-note=\"note {i}\" id=\"{id}\">\n          <mxCell style=\"{style}\" vertex=\"1\" parent=\"{parent}\">\n            <mxGeometry x=\"{x}\" y=\"{y}\" width=\"120\" height=\"60\" as=\"geometry\" />\n          </mxCell>\n        </UserObject>\n"
            ));
        } else {
            cells.push_str(&format!(
                "        <mxCell id=\"{id}\" value=\"{label}\" style=\"{style}\" vertex=\"1\" parent=\"{parent}\">\n          <mxGeometry x=\"{x}\" y=\"{y}\" width=\"120\" height=\"60\" as=\"geometry\" />\n        </mxCell>\n"
            ));
        }
        vertices.push(id);
    }
    if vertices.len() >= 2 {
        for j in 0..rng.below(8) {
            let (a, b) = (
                &vertices[rng.below(vertices.len())],
                &vertices[rng.below(vertices.len())],
            );
            let style = [
                "html=1;",
                "html=1;dashed=1;",
                "html=1;endArrow=none;strokeWidth=4;",
                "html=1;startArrow=oval;endArrow=oval;",
            ][rng.below(4)];
            let value = if rng.below(2) == 0 { "" } else { "calls" };
            cells.push_str(&format!(
                "        <mxCell id=\"e{j}\" value=\"{value}\" style=\"{style}\" edge=\"1\" parent=\"1\" source=\"{a}\" target=\"{b}\">\n          <mxGeometry relative=\"1\" as=\"geometry\" />\n        </mxCell>\n"
            ));
            if value.is_empty() && rng.below(2) == 0 {
                cells.push_str(&format!(
                    "        <mxCell id=\"e{j}-label\" value=\"via queue\" style=\"edgeLabel;html=1;\" vertex=\"1\" connectable=\"0\" parent=\"e{j}\">\n          <mxGeometry x=\"-0.1\" relative=\"1\" as=\"geometry\" />\n        </mxCell>\n"
                ));
            }
        }
    }
    let model = format!(
        "<mxGraphModel dx=\"800\" dy=\"600\" grid=\"1\">\n      <root>\n        <mxCell id=\"0\" />\n        <mxCell id=\"1\" parent=\"0\" />\n{cells}      </root>\n    </mxGraphModel>"
    );
    let body = if compressed {
        format!(
            "<diagram id=\"g\" name=\"Generated\">{}</diagram>",
            support::drawio_compress(&model)
        )
    } else {
        format!("<diagram id=\"g\" name=\"Generated\">\n    {model}\n  </diagram>")
    };
    format!("<mxfile host=\"test\">\n  {body}\n</mxfile>\n")
}

fn random_edit(d: &model::Diagram, rng: &mut Rng, step: usize) -> Edit {
    let node = |rng: &mut Rng| d.nodes[rng.below(d.nodes.len())].id.clone();
    let movable: Vec<String> = d.layout.keys().cloned().collect();
    match rng.below(9) {
        0 if !d.nodes.is_empty() => Edit::SetLabel {
            id: node(rng),
            label: format!("Renamed <{step}> & co"),
        },
        1 if !d.nodes.is_empty() => Edit::RemoveNode { id: node(rng) },
        2 if !movable.is_empty() => Edit::Move {
            id: movable[rng.below(movable.len())].clone(),
            x: rng.below(500) as f64,
            y: rng.below(500) as f64,
        },
        3 if !d.nodes.is_empty() => Edit::Annotate {
            target: Target::Node(node(rng)),
            annotation: if rng.below(3) == 0 {
                Annotation::default()
            } else {
                Annotation {
                    code: vec![CodeRef::parse(&format!("src/m{step}.rs:{}", step + 1))],
                    note: Some(format!("step {step}\nsecond \"line\"")),
                    ..Annotation::default()
                }
            },
        },
        4 if !d.edges.is_empty() => Edit::RemoveEdge(EdgeRef::of(d, rng.below(d.edges.len()))),
        5 if !d.edges.is_empty() => Edit::SetEdgeLabel {
            edge: EdgeRef::of(d, rng.below(d.edges.len())),
            label: if rng.below(2) == 0 {
                None
            } else {
                Some(format!("label {step}"))
            },
        },
        6 if d.nodes.len() >= 2 => Edit::AddEdge {
            from: node(rng),
            to: node(rng),
            label: Some("new".into()),
        },
        7 => Edit::Annotate {
            target: Target::Diagram,
            annotation: Annotation {
                kind: Some(format!("k{step}")),
                ..Annotation::default()
            },
        },
        _ => Edit::AddNode {
            id: format!("added{step}"),
            label: format!("Added {step}"),
            shape: Some(["cylinder", "diamond", "rounded"][step % 3].into()),
        },
    }
}

#[test]
fn generated_pages_survive_long_edit_sequences() {
    let seeds: u64 = std::env::var("SCHEMATLAS_GEN_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40);
    let mut checked = 0;
    for seed in 1..=seeds {
        for compressed in [false, true] {
            let mut text = generate(seed, compressed);
            let mut rng = Rng(seed + 17);
            for step in 0..20 {
                let d = drawio::parse(&text)
                    .unwrap_or_else(|e| panic!("seed {seed}: {e}\n{text}"))
                    .remove(0)
                    .diagram;
                let edit = random_edit(&d, &mut rng, step);
                let Ok(expected) = model::apply(&d, &edit) else {
                    assert!(
                        drawio::apply(&text, 0, &edit).is_err(),
                        "seed {seed} step {step}: {edit:?}"
                    );
                    continue;
                };
                let out = drawio::apply(&text, 0, &edit)
                    .unwrap_or_else(|e| panic!("seed {seed} step {step} {edit:?}: {e}\n{text}"));
                let after = drawio::parse(&out).unwrap().remove(0).diagram;
                assert!(
                    after.same_as(&expected),
                    "seed {seed} step {step}: {edit:?}\n{out}"
                );
                if compressed {
                    assert!(!out.contains("<mxCell"), "the page stays compressed");
                }
                text = out;
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked}");
}
