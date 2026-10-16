//! Sequence diagrams laid out the way Mermaid draws them: one column per participant, spaced so
//! every message label fits between its two lifelines, and one row per step, top to bottom.
//! Pure geometry, so it runs anywhere and is tested natively.
use schematlas_parser::model::{Arrow, Diagram, NotePlacement, Step};
use std::collections::HashMap;

/// Space around the drawing.
pub const MARGIN: f64 = 32.0;
/// Participant boxes.
pub const BOX_MIN_WIDTH: f64 = 132.0;
pub const BOX_MAX_WIDTH: f64 = 240.0;
pub const BOX_HEIGHT: f64 = 46.0;
const BOX_LINE: f64 = 16.0;
/// Least distance between two neighbouring lifelines.
const MIN_GAP: f64 = 168.0;
/// Room a message label needs beyond its own width.
const LABEL_PAD: f64 = 44.0;
/// Longest message label before it is shortened.
pub const LABEL_MAX_WIDTH: f64 = 360.0;
const MESSAGE_ROW: f64 = 46.0;
const SELF_ROW: f64 = 64.0;
const SELF_WIDTH: f64 = 34.0;
const NOTE_LINE: f64 = 17.0;
const NOTE_MIN_WIDTH: f64 = 120.0;
const NOTE_MAX_WIDTH: f64 = 260.0;
const FRAME_HEADER: f64 = 34.0;
const BRANCH_ROW: f64 = 30.0;
const FRAME_PAD: f64 = 18.0;
/// Activation bars, nested ones offset to the right.
pub const BAR_WIDTH: f64 = 10.0;

/// Rough width of text at 12px, generous enough that labels never collide.
pub fn text_width(text: &str) -> f64 {
    text.chars()
        .map(|c| match c {
            'i' | 'l' | 'j' | 'I' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 3.4,
            'f' | 't' | 'r' | ' ' | '(' | ')' | '[' | ']' | '/' | '-' => 4.6,
            'm' | 'w' | 'M' | 'W' | '@' => 10.2,
            c if c.is_ascii_uppercase() || c.is_ascii_digit() => 7.8,
            c if c.is_ascii() => 6.8,
            _ => 12.0,
        })
        .sum()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Participant {
    pub id: String,
    /// Lifeline position.
    pub x: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    pub index: usize,
    pub from: String,
    pub to: String,
    pub label: String,
    pub dotted: bool,
    pub arrow: Arrow,
    /// Line ends, after activation bars.
    pub x1: f64,
    pub x2: f64,
    pub y: f64,
    /// A message to itself loops out to the right and back.
    pub to_self: bool,
    /// Shown when the diagram numbers its messages.
    pub number: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub actor: String,
    pub x: f64,
    pub y1: f64,
    pub y2: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// `loop`, `alt`, `opt`, `par`, `critical`, `break` or `rect`.
    pub block: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// `else`/`and`/`option` dividers: their y and label.
    pub branches: Vec<(f64, String)>,
    pub depth: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SequenceLayout {
    pub width: f64,
    pub height: f64,
    pub participants: Vec<Participant>,
    pub messages: Vec<Message>,
    pub notes: Vec<Note>,
    pub bars: Vec<Bar>,
    pub frames: Vec<Frame>,
    /// Where lifelines start (under the top boxes) and end (above the bottom ones).
    pub lifeline_top: f64,
    pub lifeline_bottom: f64,
}

fn box_size(label: &str) -> (f64, f64) {
    let lines = crate::label::lines(label);
    let widest = lines
        .iter()
        .map(|l| text_width(&l.iter().map(|r| r.text.as_str()).collect::<String>()) * 1.12)
        .fold(0.0, f64::max);
    let width = (widest + 32.0).clamp(BOX_MIN_WIDTH, BOX_MAX_WIDTH);
    let height = BOX_HEIGHT + lines.len().saturating_sub(1) as f64 * BOX_LINE;
    (width, height)
}

fn note_size(text: &str) -> (f64, f64) {
    let lines = crate::label::lines(text);
    let widest = lines
        .iter()
        .map(|l| text_width(&l.iter().map(|r| r.text.as_str()).collect::<String>()))
        .fold(0.0, f64::max);
    let width = (widest + 24.0).clamp(NOTE_MIN_WIDTH, NOTE_MAX_WIDTH);
    // Long single lines wrap inside the note.
    let wrapped: usize = lines
        .iter()
        .map(|l| {
            let w = text_width(&l.iter().map(|r| r.text.as_str()).collect::<String>());
            ((w / (NOTE_MAX_WIDTH - 24.0)).ceil() as usize).max(1)
        })
        .sum();
    (width, wrapped.max(1) as f64 * NOTE_LINE + 16.0)
}

/// Gaps between neighbouring lifelines, widened until every label and note fits.
fn gaps(diagram: &Diagram, index: &HashMap<&str, usize>, widths: &[f64]) -> Vec<f64> {
    let n = widths.len();
    let mut gaps: Vec<f64> = (0..n.saturating_sub(1))
        .map(|i| ((widths[i] + widths[i + 1]) / 2.0 + 28.0).max(MIN_GAP))
        .collect();
    let Some(sequence) = &diagram.sequence else {
        return gaps;
    };
    // Room right of the last lifeline, for self messages and notes there.
    let mut right_room = 0.0_f64;
    let mut left_room = 0.0_f64;
    let need = |from: usize, to: usize, width: f64, gaps: &mut Vec<f64>| {
        let (a, b) = (from.min(to), from.max(to));
        if a == b || b > gaps.len() {
            return;
        }
        let have: f64 = gaps[a..b].iter().sum();
        if have < width {
            let extra = (width - have) / (b - a) as f64;
            for g in &mut gaps[a..b] {
                *g += extra;
            }
        }
    };
    for step in &sequence.steps {
        match step {
            Step::Message {
                from, to, label, ..
            } => {
                let (Some(&a), Some(&b)) = (index.get(from.as_str()), index.get(to.as_str()))
                else {
                    continue;
                };
                let width = text_width(label).min(LABEL_MAX_WIDTH) + LABEL_PAD;
                if a == b {
                    // The label sits right of the loop.
                    let width = width + SELF_WIDTH;
                    if a + 1 < n {
                        need(a, a + 1, width, &mut gaps);
                    } else {
                        right_room = right_room.max(width);
                    }
                } else {
                    need(a, b, width, &mut gaps);
                }
            }
            Step::Note {
                over,
                placement,
                text,
            } => {
                let (width, _) = note_size(text);
                let ids: Vec<usize> = over
                    .iter()
                    .filter_map(|id| index.get(id.as_str()).copied())
                    .collect();
                let Some(&first) = ids.first() else { continue };
                match placement {
                    NotePlacement::RightOf => {
                        if first + 1 < n {
                            need(first, first + 1, width + 24.0, &mut gaps);
                        } else {
                            right_room = right_room.max(width + 24.0);
                        }
                    }
                    NotePlacement::LeftOf => {
                        if first > 0 {
                            need(first - 1, first, width + 24.0, &mut gaps);
                        } else {
                            left_room = left_room.max(width + 24.0);
                        }
                    }
                    NotePlacement::Over => {
                        let last = *ids.last().unwrap_or(&first);
                        if last != first {
                            need(first, last, width - 40.0, &mut gaps);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    gaps.push(right_room);
    gaps.insert(0, left_room);
    gaps
}

/// Lay out a sequence diagram. A diagram without a sequence gives an empty layout.
pub fn layout(diagram: &Diagram) -> SequenceLayout {
    let Some(sequence) = &diagram.sequence else {
        return SequenceLayout::default();
    };
    let sizes: Vec<(f64, f64)> = diagram.nodes.iter().map(|n| box_size(&n.label)).collect();
    let widths: Vec<f64> = sizes.iter().map(|s| s.0).collect();
    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut room = gaps(diagram, &index, &widths);
    let left_room = room.remove(0);
    let right_room = room.pop().unwrap_or(0.0);
    let header = sizes.iter().map(|s| s.1).fold(BOX_HEIGHT, f64::max);
    let mut x = MARGIN + left_room + widths.first().copied().unwrap_or(0.0) / 2.0;
    let mut participants = Vec::with_capacity(diagram.nodes.len());
    for (i, node) in diagram.nodes.iter().enumerate() {
        if i > 0 {
            x += room[i - 1];
        }
        participants.push(Participant {
            id: node.id.clone(),
            x,
            width: widths[i],
            height: header,
        });
    }
    let center = |id: &str| index.get(id).map(|&i| participants[i].x);

    let mut out = SequenceLayout {
        lifeline_top: MARGIN + header,
        ..SequenceLayout::default()
    };
    let mut y = MARGIN + header + 26.0;
    // Open activations per participant: the y each started at.
    let mut active: HashMap<String, Vec<f64>> = HashMap::new();
    let depth = |active: &HashMap<String, Vec<f64>>, id: &str| active.get(id).map_or(0, Vec::len);
    let mut last_line = y;
    // Open frames: index into `out.frames`, and the participants they touch.
    let mut open: Vec<(usize, Option<(f64, f64)>)> = vec![];
    let touch = |open: &mut Vec<(usize, Option<(f64, f64)>)>, x0: f64, x1: f64| {
        for (_, span) in open.iter_mut() {
            *span = Some(match span {
                Some((a, b)) => (a.min(x0), b.max(x1)),
                None => (x0, x1),
            });
        }
    };
    let mut number = 0;
    for (index, step) in sequence.steps.iter().enumerate() {
        match step {
            Step::Message {
                from,
                to,
                label,
                dotted,
                arrow,
            } => {
                let (Some(a), Some(b)) = (center(from), center(to)) else {
                    continue;
                };
                number += 1;
                let to_self = from == to;
                let row = if to_self { SELF_ROW } else { MESSAGE_ROW };
                let line = y + 22.0;
                // Lines start and end at the edge of an active bar.
                let edge = |id: &str, toward: f64, at: f64| {
                    let d = depth(&active, id);
                    if d == 0 {
                        at
                    } else {
                        let half = BAR_WIDTH / 2.0 + (d - 1) as f64 * (BAR_WIDTH / 2.0);
                        if toward >= at {
                            at + half
                        } else {
                            at - half
                        }
                    }
                };
                let (x1, x2) = if to_self {
                    (edge(from, a + 1.0, a), edge(from, a + 1.0, a))
                } else {
                    (edge(from, b, a), edge(to, a, b))
                };
                out.messages.push(Message {
                    index,
                    from: from.clone(),
                    to: to.clone(),
                    label: label.clone(),
                    dotted: *dotted,
                    arrow: *arrow,
                    x1,
                    x2,
                    y: line,
                    to_self,
                    number: sequence.autonumber.then_some(number),
                });
                let reach = if to_self {
                    a + SELF_WIDTH + text_width(label).min(LABEL_MAX_WIDTH) + 12.0
                } else {
                    a.max(b)
                };
                touch(&mut open, a.min(b), reach);
                last_line = line;
                y += row;
            }
            Step::Note {
                over,
                placement,
                text,
            } => {
                let xs: Vec<f64> = over.iter().filter_map(|id| center(id)).collect();
                let Some(&first) = xs.first() else { continue };
                let last = *xs.last().unwrap_or(&first);
                let (mut width, height) = note_size(text);
                let x = match placement {
                    NotePlacement::RightOf => first + 14.0,
                    NotePlacement::LeftOf => first - 14.0 - width,
                    NotePlacement::Over if xs.len() > 1 => {
                        width = width.max((last - first).abs() + 60.0);
                        first.min(last) - 30.0
                    }
                    NotePlacement::Over => first - width / 2.0,
                };
                out.notes.push(Note {
                    text: text.clone(),
                    x,
                    y: y + 6.0,
                    width,
                    height,
                });
                touch(&mut open, x, x + width);
                y += height + 18.0;
            }
            Step::Activate { actor } => {
                // A bar starts at the message that activated it.
                active.entry(actor.clone()).or_default().push(last_line);
            }
            Step::Deactivate { actor } => {
                if let (Some(start), Some(x)) =
                    (active.get_mut(actor).and_then(Vec::pop), center(actor))
                {
                    let d = depth(&active, actor);
                    out.bars.push(Bar {
                        actor: actor.clone(),
                        x: x - BAR_WIDTH / 2.0 + d as f64 * (BAR_WIDTH / 2.0),
                        y1: start,
                        y2: last_line.max(start + 12.0),
                    });
                }
            }
            Step::Start { block, label } => {
                out.frames.push(Frame {
                    block: block.clone(),
                    label: label.clone(),
                    x: 0.0,
                    y,
                    width: 0.0,
                    height: 0.0,
                    branches: vec![],
                    depth: open.len(),
                });
                open.push((out.frames.len() - 1, None));
                y += FRAME_HEADER;
            }
            Step::Branch { label } => {
                if let Some((i, _)) = open.last() {
                    out.frames[*i].branches.push((y + 4.0, label.clone()));
                }
                y += BRANCH_ROW;
            }
            Step::End => {
                if let Some((i, span)) = open.pop() {
                    let all = (
                        participants.first().map_or(0.0, |p| p.x),
                        participants.last().map_or(0.0, |p| p.x),
                    );
                    let (x0, x1) = span.unwrap_or(all);
                    let pad = FRAME_PAD * (open.len() as f64 + 1.0).min(3.0) + 46.0;
                    let frame = &mut out.frames[i];
                    frame.x = x0 - pad;
                    frame.width = (x1 - x0) + 2.0 * pad;
                    frame.height = y + 10.0 - frame.y;
                    // The enclosing frames contain this one.
                    let (fx0, fx1) = (frame.x, frame.x + frame.width);
                    touch(&mut open, fx0 + 1.0, fx1 - 1.0);
                    y += 22.0;
                }
            }
        }
    }
    // Close what the file left open.
    for (actor, starts) in active {
        if let Some(x) = center(&actor) {
            for (d, start) in starts.into_iter().enumerate() {
                out.bars.push(Bar {
                    actor: actor.clone(),
                    x: x - BAR_WIDTH / 2.0 + d as f64 * (BAR_WIDTH / 2.0),
                    y1: start,
                    y2: y,
                });
            }
        }
    }
    while let Some((i, span)) = open.pop() {
        let (x0, x1) = span.unwrap_or((
            participants.first().map_or(0.0, |p| p.x),
            participants.last().map_or(0.0, |p| p.x),
        ));
        let frame = &mut out.frames[i];
        frame.x = x0 - 46.0;
        frame.width = x1 - x0 + 92.0;
        frame.height = y - frame.y;
    }
    out.lifeline_bottom = y + 10.0;
    let right = participants
        .iter()
        .map(|p| p.x + p.width / 2.0)
        .chain(out.notes.iter().map(|n| n.x + n.width))
        .chain(out.frames.iter().map(|f| f.x + f.width))
        .chain(
            out.messages
                .iter()
                .filter(|m| m.to_self)
                .map(|m| m.x1 + SELF_WIDTH + text_width(&m.label).min(LABEL_MAX_WIDTH) + 16.0),
        )
        .fold(0.0, f64::max)
        .max(x + right_room);
    // Notes and frames left of the first lifeline shift everything right.
    let left = out
        .notes
        .iter()
        .map(|n| n.x)
        .chain(out.frames.iter().map(|f| f.x))
        .chain(participants.iter().map(|p| p.x - p.width / 2.0))
        .fold(f64::INFINITY, f64::min);
    let shift = if left < MARGIN { MARGIN - left } else { 0.0 };
    if shift > 0.0 {
        for p in &mut participants {
            p.x += shift;
        }
        for m in &mut out.messages {
            m.x1 += shift;
            m.x2 += shift;
        }
        for n in &mut out.notes {
            n.x += shift;
        }
        for b in &mut out.bars {
            b.x += shift;
        }
        for f in &mut out.frames {
            f.x += shift;
        }
    }
    out.participants = participants;
    out.width = right + shift + MARGIN;
    out.height = out.lifeline_bottom + header + MARGIN;
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use schematlas_parser::model::{Node, Sequence};

    fn diagram(participants: &[(&str, &str)], steps: Vec<Step>) -> Diagram {
        Diagram {
            kind: "sequence".into(),
            nodes: participants
                .iter()
                .map(|(id, label)| Node {
                    id: (*id).into(),
                    label: (*label).into(),
                    ..Node::default()
                })
                .collect(),
            sequence: Some(Sequence {
                autonumber: true,
                steps,
            }),
            ..Diagram::default()
        }
    }
    fn message(from: &str, to: &str, label: &str) -> Step {
        Step::Message {
            from: from.into(),
            to: to.into(),
            label: label.into(),
            dotted: false,
            arrow: Arrow::Filled,
        }
    }

    #[test]
    fn columns_never_overlap_and_labels_fit_between_lifelines() {
        let long = "POST /orders with the whole basket and the payment intent";
        let d = diagram(
            &[("u", "User"), ("w", "Web app"), ("a", "Orders API")],
            vec![message("u", "w", "Checkout"), message("w", "a", long)],
        );
        let l = layout(&d);
        for pair in l.participants.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(a.x + a.width / 2.0 < b.x - b.width / 2.0, "{a:?} {b:?}");
        }
        let (w, a) = (&l.participants[1], &l.participants[2]);
        assert!(a.x - w.x >= text_width(long) + 40.0);
        assert_eq!(l.messages.len(), 2);
        assert!(l.messages[0].y < l.messages[1].y);
        assert_eq!(l.messages[1].number, Some(2));
        assert!(l.width >= a.x + a.width / 2.0);
    }

    #[test]
    fn frames_nest_and_bars_follow_activations() {
        let d = diagram(
            &[("w", "Web"), ("a", "API"), ("d", "DB")],
            vec![
                message("w", "a", "POST /orders"),
                Step::Activate { actor: "a".into() },
                Step::Start {
                    block: "loop".into(),
                    label: "every item".into(),
                },
                Step::Start {
                    block: "alt".into(),
                    label: "in stock".into(),
                },
                message("a", "d", "reserve"),
                Step::Branch {
                    label: "sold out".into(),
                },
                message("a", "w", "409"),
                Step::End,
                Step::End,
                Step::Deactivate { actor: "a".into() },
                message("a", "a", "log"),
            ],
        );
        let l = layout(&d);
        let (outer, inner) = (&l.frames[0], &l.frames[1]);
        assert_eq!((outer.depth, inner.depth), (0, 1));
        assert!(outer.x < inner.x && outer.x + outer.width > inner.x + inner.width);
        assert!(outer.y < inner.y && outer.y + outer.height > inner.y + inner.height);
        assert_eq!(inner.branches.len(), 1);
        let bar = &l.bars[0];
        assert_eq!(bar.actor, "a");
        assert_eq!(bar.y1, l.messages[0].y);
        // The reply inside the bar starts at its edge, not at the lifeline.
        let api = l.participants[1].x;
        assert!(l.messages[2].x1 < api);
        assert!(l.messages[3].to_self);
        assert!(l.width > l.messages[3].x1 + SELF_WIDTH);
    }

    #[test]
    fn notes_left_of_the_first_participant_stay_in_view() {
        let d = diagram(
            &[("a", "API"), ("b", "DB")],
            vec![
                Step::Note {
                    over: vec!["a".into()],
                    placement: NotePlacement::LeftOf,
                    text: "A fairly long note on the left".into(),
                },
                message("a", "b", "query"),
            ],
        );
        let l = layout(&d);
        assert!(l.notes[0].x >= MARGIN - 0.5, "{:?}", l.notes[0]);
        assert!(l.notes[0].x + l.notes[0].width <= l.participants[0].x);
    }
}
