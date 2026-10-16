//! Class, entity-relationship, state and sequence diagrams, read into the graph model. They take
//! annotations; structural edits are reserved to flowcharts for now.
use super::text;
use crate::model::{
    Arrow, Diagram, Edge, Group, Head, Node, NotePlacement, Sequence, Step, Stroke,
};
use serde_json::{Map, Value};

/// Entries of a JSON object in the order merman created them (its ids end in a counter).
fn in_order(map: Option<&Map<String, Value>>, id_key: &str) -> Vec<(String, Value)> {
    let mut entries: Vec<(String, Value)> = map
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let counter = |v: &Value| {
        v[id_key]
            .as_str()
            .and_then(|id| id.rsplit('-').next())
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    };
    entries.sort_by_key(|(_, v)| counter(v));
    entries
}

fn uml_head(kind: i64) -> Head {
    match kind {
        0 => Head::Diamond,
        1 => Head::Triangle,
        2 => Head::FilledDiamond,
        4 => Head::Circle,
        -1 => Head::None,
        _ => Head::Arrow,
    }
}
fn side_label(v: &Value) -> Option<String> {
    text(v).filter(|t| t != "none")
}

pub(super) fn class(model: &Value) -> Diagram {
    let mut d = Diagram {
        kind: "class".into(),
        direction: text(&model["direction"]),
        ..Diagram::default()
    };
    for (id, c) in in_order(model["classes"].as_object(), "domId") {
        let mut details: Vec<String> = c["annotations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|a| a.as_str())
            .map(|a| format!("«{a}»"))
            .collect();
        for key in ["members", "methods"] {
            details.extend(
                c[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| text(&m["displayText"])),
            );
        }
        d.nodes.push(Node {
            label: text(&c["label"]).unwrap_or_else(|| id.clone()),
            shape: Some("class".into()),
            link: text(&c["link"]),
            details,
            id,
            ..Node::default()
        });
    }
    for r in model["relations"].as_array().into_iter().flatten() {
        let (a, b) = (
            r["id1"].as_str().unwrap_or_default(),
            r["id2"].as_str().unwrap_or_default(),
        );
        let (t1, t2) = (
            r["relation"]["type1"].as_i64().unwrap_or(-1),
            r["relation"]["type2"].as_i64().unwrap_or(-1),
        );
        let stroke = if r["relation"]["lineType"].as_i64() == Some(1) {
            Stroke::Dotted
        } else {
            Stroke::Normal
        };
        let (l1, l2) = (
            side_label(&r["relationTitle1"]),
            side_label(&r["relationTitle2"]),
        );
        // The marker belongs to the end it is drawn at: `Parent <|-- Child` points at Parent.
        let (from, to, head, tail_label, head_label) = if t1 != -1 && t2 == -1 {
            (b, a, uml_head(t1), l2, l1)
        } else {
            (a, b, uml_head(t2), l1, l2)
        };
        d.edges.push(Edge {
            from: from.to_owned(),
            to: to.to_owned(),
            label: text(&r["title"]),
            stroke,
            head,
            both_ends: t1 != -1 && t2 != -1,
            tail_label,
            head_label,
            ..Edge::default()
        });
    }
    d
}

fn cardinality(v: &Value) -> Option<String> {
    Some(
        match v.as_str()? {
            "ONLY_ONE" => "1",
            "ZERO_OR_ONE" => "0..1",
            "ZERO_OR_MORE" => "0..n",
            "ONE_OR_MORE" => "1..n",
            "MD_PARENT" => "parent",
            other => other,
        }
        .to_owned(),
    )
}

pub(super) fn er(model: &Value) -> Diagram {
    let mut d = Diagram {
        kind: "er".into(),
        direction: text(&model["direction"]),
        ..Diagram::default()
    };
    let entities = in_order(model["entities"].as_object(), "id");
    for (name, e) in &entities {
        let details = e["attributes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|a| {
                let mut line = format!(
                    "{} {}",
                    a["type"].as_str().unwrap_or(""),
                    a["name"].as_str().unwrap_or("")
                )
                .trim()
                .to_owned();
                for key in a["keys"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|k| k.as_str())
                {
                    line.push(' ');
                    line.push_str(key);
                }
                if let Some(comment) = text(&a["comment"]) {
                    line.push_str(&format!(" \"{comment}\""));
                }
                line
            })
            .collect();
        d.nodes.push(Node {
            id: name.clone(),
            label: text(&e["alias"])
                .or_else(|| text(&e["label"]))
                .unwrap_or_else(|| name.clone()),
            shape: Some("entity".into()),
            details,
            ..Node::default()
        });
    }
    let name_of = |id: &str| {
        entities
            .iter()
            .find(|(_, e)| e["id"].as_str() == Some(id))
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| id.to_owned())
    };
    for r in model["relationships"].as_array().into_iter().flatten() {
        let spec = &r["relSpec"];
        d.edges.push(Edge {
            from: name_of(r["entityA"].as_str().unwrap_or_default()),
            to: name_of(r["entityB"].as_str().unwrap_or_default()),
            label: text(&r["roleA"]),
            stroke: if spec["relType"].as_str() == Some("NON_IDENTIFYING") {
                Stroke::Dotted
            } else {
                Stroke::Normal
            },
            head: Head::None,
            // merman names each cardinality after the entity on the other side.
            tail_label: cardinality(&spec["cardB"]),
            head_label: cardinality(&spec["cardA"]),
            ..Edge::default()
        });
    }
    d
}

pub(super) fn state(model: &Value) -> Diagram {
    let mut d = Diagram {
        kind: "state".into(),
        direction: text(&model["direction"]),
        ..Diagram::default()
    };
    let nodes: Vec<&Value> = model["nodes"].as_array().into_iter().flatten().collect();
    for n in &nodes {
        let id = n["id"].as_str().unwrap_or_default().to_owned();
        let parent = text(&n["parentId"]);
        if n["isGroup"].as_bool() == Some(true) {
            d.groups.push(Group {
                title: text(&n["label"]).unwrap_or_else(|| id.clone()),
                parent,
                id,
                ..Group::default()
            });
            continue;
        }
        let (shape, label) = match n["shape"].as_str() {
            Some("stateStart") => (Some("start"), "start".to_owned()),
            Some("stateEnd") => (Some("end"), "end".to_owned()),
            Some("choice") => (Some("diamond"), text(&n["label"]).unwrap_or_default()),
            Some("fork" | "join") => (Some("bar"), text(&n["label"]).unwrap_or_default()),
            Some("note") => (Some("note"), text(&n["label"]).unwrap_or_default()),
            _ => (None, text(&n["label"]).unwrap_or_else(|| id.clone())),
        };
        d.nodes.push(Node {
            id,
            label,
            shape: shape.map(str::to_owned),
            ..Node::default()
        });
    }
    for g in &mut d.groups {
        g.nodes = nodes
            .iter()
            .filter(|n| {
                n["isGroup"].as_bool() != Some(true)
                    && n["parentId"].as_str() == Some(g.id.as_str())
            })
            .filter_map(|n| n["id"].as_str().map(str::to_owned))
            .collect();
    }
    for e in model["edges"].as_array().into_iter().flatten() {
        d.edges.push(Edge {
            from: e["start"].as_str().unwrap_or_default().to_owned(),
            to: e["end"].as_str().unwrap_or_default().to_owned(),
            label: text(&e["label"]),
            ..Edge::default()
        });
    }
    d
}

/// A sequence diagram: participants as nodes, messages as edges, and every line in order as a
/// step (messages, notes, activations and frames).
pub(super) fn sequence(model: &Value) -> Diagram {
    let mut d = Diagram {
        kind: "sequence".into(),
        title: text(&model["title"]),
        ..Diagram::default()
    };
    let actors = &model["actors"];
    for id in model["actorOrder"].as_array().into_iter().flatten() {
        let Some(id) = id.as_str() else { continue };
        let actor = &actors[id];
        let shape = match actor["type"].as_str() {
            Some("actor") => Some("actor"),
            Some("database") => Some("cylinder"),
            Some("queue") => Some("queue"),
            Some("collections") => Some("collections"),
            Some(kind @ ("boundary" | "control" | "entity")) => Some(kind),
            _ => None,
        };
        d.nodes.push(Node {
            id: id.to_owned(),
            label: text(&actor["description"]).unwrap_or_else(|| id.to_owned()),
            shape: shape.map(str::to_owned),
            ..Node::default()
        });
    }
    let mut sequence = Sequence::default();
    let name = |v: &Value| v.as_str().map(str::to_owned);
    for m in model["messages"].as_array().into_iter().flatten() {
        let label = m["message"].as_str().unwrap_or_default().trim().to_owned();
        let (from, to) = (name(&m["from"]), name(&m["to"]));
        let kind = m["type"].as_i64().unwrap_or(-1);
        let message = |dotted: bool, arrow: Arrow| match (&from, &to) {
            (Some(from), Some(to)) => Some(Step::Message {
                from: from.clone(),
                to: to.clone(),
                label: label.clone(),
                dotted,
                arrow,
            }),
            _ => None,
        };
        let step = match kind {
            0 => message(false, Arrow::Filled),
            1 => message(true, Arrow::Filled),
            3 => message(false, Arrow::Cross),
            4 => message(true, Arrow::Cross),
            5 => message(false, Arrow::Open),
            6 => message(true, Arrow::Open),
            24 => message(false, Arrow::Async),
            25 => message(true, Arrow::Async),
            33 => message(false, Arrow::Both),
            34 => message(true, Arrow::Both),
            2 => {
                let mut over: Vec<String> = from.iter().cloned().collect();
                if let Some(to) = &to {
                    if !over.contains(to) {
                        over.push(to.clone());
                    }
                }
                let placement = match m["placement"].as_i64() {
                    Some(0) => NotePlacement::LeftOf,
                    Some(1) => NotePlacement::RightOf,
                    _ => NotePlacement::Over,
                };
                Some(Step::Note {
                    over,
                    placement,
                    text: label.clone(),
                })
            }
            17 => from.clone().map(|actor| Step::Activate { actor }),
            18 => from.clone().map(|actor| Step::Deactivate { actor }),
            10 | 12 | 15 | 19 | 22 | 27 | 30 | 32 => Some(Step::Start {
                block: match kind {
                    10 => "loop",
                    12 => "alt",
                    15 => "opt",
                    19 | 32 => "par",
                    22 => "rect",
                    27 => "critical",
                    _ => "break",
                }
                .into(),
                label: label.clone(),
            }),
            13 | 20 | 28 => Some(Step::Branch {
                label: label.clone(),
            }),
            11 | 14 | 16 | 21 | 23 | 29 | 31 => Some(Step::End),
            26 => {
                sequence.autonumber = true;
                None
            }
            // Newer arrow styles (half arrows, central connections) read as plain messages.
            41..=61 => message((51..=58).contains(&kind), Arrow::Filled),
            _ => None,
        };
        if let Some(Step::Message {
            from,
            to,
            label,
            dotted,
            ..
        }) = &step
        {
            d.edges.push(Edge {
                from: from.clone(),
                to: to.clone(),
                label: (!label.is_empty()).then(|| label.clone()),
                stroke: if *dotted {
                    Stroke::Dotted
                } else {
                    Stroke::Normal
                },
                ..Edge::default()
            });
        }
        sequence.steps.extend(step);
    }
    d.sequence = Some(sequence);
    d
}
