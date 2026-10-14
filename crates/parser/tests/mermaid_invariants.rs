//! Properties that must hold for every fixture: reading is stable, writing without edits is the
//! identity, and editing the text agrees with editing the model.
mod support;
use schematlas_parser::mermaid;
use schematlas_parser::model::Edit;
use support::{check, every_edit, fixtures, Rng};

fn editable(name: &str) -> bool {
    // The empty diagram has nothing to edit; class, ER and state diagrams take annotations only.
    !name.starts_with("13-") && !name.starts_with("2")
}

#[test]
fn every_fixture_parses_deterministically() {
    for (name, src) in fixtures() {
        let a = mermaid::parse(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let b = mermaid::parse(&src).unwrap();
        assert_eq!(a.diagram, b.diagram, "{name}");
    }
}

#[test]
fn every_single_edit_on_every_fixture_agrees_with_the_model() {
    let mut checked = 0;
    for (name, src) in fixtures().into_iter().filter(|(n, _)| editable(n)) {
        let d = mermaid::parse(&src).unwrap().diagram;
        for edit in every_edit(&d) {
            if check(&name, &src, &edit).is_some() {
                checked += 1;
            }
        }
    }
    assert!(checked > 300, "only {checked} edits checked");
}

#[test]
fn random_edit_sequences_stay_valid_and_consistent() {
    for (name, src) in fixtures().into_iter().filter(|(n, _)| editable(n)) {
        for seed in 1..=12u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ name.len() as u64);
            let mut text = src.clone();
            for step in 0..15 {
                let d = mermaid::parse(&text).unwrap().diagram;
                let mut edits = every_edit(&d);
                edits.push(Edit::AddNode {
                    id: format!("n{seed}_{step}"),
                    label: format!("Node {step}"),
                    shape: None,
                });
                if d.nodes.len() >= 2 {
                    let a = &d.nodes[rng.below(d.nodes.len())].id;
                    let b = &d.nodes[rng.below(d.nodes.len())].id;
                    edits.push(Edit::AddEdge {
                        from: a.clone(),
                        to: b.clone(),
                        label: None,
                    });
                }
                let edit = edits.swap_remove(rng.below(edits.len()));
                if let Some(next) = check(&format!("{name} seed {seed} step {step}"), &text, &edit)
                {
                    text = next;
                }
            }
        }
    }
}

#[test]
fn edits_never_touch_lines_unrelated_to_them() {
    for (name, src) in fixtures().into_iter().filter(|(n, _)| editable(n)) {
        let d = mermaid::parse(&src).unwrap().diagram;
        for n in &d.nodes {
            let out = mermaid::apply(
                &src,
                &Edit::SetLabel {
                    id: n.id.clone(),
                    label: "Z".into(),
                },
            )
            .unwrap();
            let (removed, added) = support::changed_lines(&src, &out);
            // A label written over several lines is one change spread over those lines.
            let span = n.label.lines().count().max(1);
            assert!(
                removed.len() <= span && added.len() <= 1,
                "{name} rename {}: -{removed:?} +{added:?}",
                n.id
            );
        }
        let out = mermaid::apply(
            &src,
            &Edit::AddNode {
                id: "zz".into(),
                label: "Z".into(),
                shape: None,
            },
        )
        .unwrap();
        let (removed, added) = support::changed_lines(&src, &out);
        assert!(
            removed.is_empty() && added.len() == 1,
            "{name} add: -{removed:?} +{added:?}"
        );
    }
}
