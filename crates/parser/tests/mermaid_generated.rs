//! Randomly generated, valid flowcharts: every construct the reader supports, mixed the way real
//! files mix them, each checked with random edits against the model.
mod support;
use schematlas_parser::mermaid;
use support::{check, every_edit, Rng};

const SHAPES: [(&str, &str); 8] = [
    ("[", "]"),
    ("(", ")"),
    ("([", "])"),
    ("[(", ")]"),
    ("((", "))"),
    ("{", "}"),
    ("{{", "}}"),
    ("[[", "]]"),
];
const LABELS: [&str; 8] = [
    "Orders API",
    "\"Cache (hot)\"",
    "Worker",
    "\"Say #quot;hi#quot;\"",
    "Payments",
    "\"a | b\"",
    "Ünïcode",
    "Search-index",
];
const ARROWS: [&str; 9] = [
    "-->", "---", "-.->", "==>", "--o", "--x", "<-->", "---->", "-.-",
];

struct Gen {
    rng: Rng,
    out: String,
    nodes: usize,
    edges: usize,
    defined: Vec<bool>,
}
impl Gen {
    fn id(&mut self) -> String {
        format!("n{}", self.rng.below(self.nodes))
    }
    /// An id, sometimes with a shape and label (each node defined at most once here).
    fn mention(&mut self) -> String {
        let i = self.rng.below(self.nodes);
        if !self.defined[i] && self.rng.below(3) == 0 {
            self.defined[i] = true;
            let (open, close) = SHAPES[self.rng.below(SHAPES.len())];
            let label = LABELS[self.rng.below(LABELS.len())];
            return format!("n{i}{open}{label}{close}");
        }
        format!("n{i}")
    }
    fn arrow(&mut self) -> String {
        let arrow = ARROWS[self.rng.below(ARROWS.len())];
        match self.rng.below(4) {
            0 => format!(
                "{arrow}|{}|",
                ["calls", "\"reads (x)\"", "writes"][self.rng.below(3)]
            ),
            1 if arrow == "-->" => "-- emits -->".into(),
            _ => arrow.into(),
        }
    }
    fn statement(&mut self, indent: &str, depth: usize) {
        let roll = self.rng.below(10);
        let line = match roll {
            0 => self.mention(),
            1 => {
                let (a, b, c) = (self.mention(), self.mention(), self.mention());
                self.edges += 2;
                format!("{a} {} {b} {} {c}", self.arrow(), self.arrow())
            }
            2 => {
                let (a, b, c) = (self.id(), self.id(), self.mention());
                self.edges += 2;
                format!("{a} & {b} {} {c}", self.arrow())
            }
            3 if depth < 2 => {
                let name = format!("g{}_{}", depth, self.rng.below(1000));
                self.out
                    .push_str(&format!("{indent}subgraph {name} [Group {name}]\n"));
                let inner = format!("{indent}  ");
                for _ in 0..=self.rng.below(3) {
                    self.statement(&inner, depth + 1);
                }
                "end".to_string()
            }
            4 => {
                let (a, b) = (self.mention(), self.mention());
                let (c, d) = (self.id(), self.id());
                self.edges += 2;
                format!("{a} {} {b}; {c} --> {d}", self.arrow())
            }
            5 => format!("%% note {}", self.rng.below(100)),
            6 => format!("class {},{} hot", self.id(), self.id()),
            7 => format!("style {} stroke-width:2px", self.id()),
            8 if self.edges > 0 => format!("linkStyle {} stroke:#f00", self.rng.below(self.edges)),
            _ => {
                let (a, b) = (self.mention(), self.mention());
                self.edges += 1;
                format!("{a} {} {b}", self.arrow())
            }
        };
        self.out.push_str(indent);
        self.out.push_str(&line);
        self.out.push('\n');
    }
}

fn generate(seed: u64) -> String {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let nodes = 3 + rng.below(10);
    let direction = ["LR", "TD", "TB", "RL", "BT"][rng.below(5)];
    let mut g = Gen {
        rng,
        out: format!("flowchart {direction}\n  classDef hot fill:#f96\n"),
        nodes,
        edges: 0,
        defined: vec![false; nodes],
    };
    for _ in 0..4 + g.rng.below(10) {
        g.statement("  ", 0);
    }
    g.out
}

#[test]
fn generated_flowcharts_parse_and_edit_consistently() {
    let mut checked = 0;
    // `SCHEMATLAS_GEN_SEEDS=2000 cargo test --release` for a long soak.
    let seeds: u64 = std::env::var("SCHEMATLAS_GEN_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    for seed in 1..=seeds {
        let src = generate(seed);
        let d = match mermaid::parse(&src) {
            Ok(doc) => doc.diagram,
            Err(e) => panic!("seed {seed}: generated diagram rejected: {e}\n{src}"),
        };
        let mut edits = every_edit(&d);
        let mut rng = Rng(seed);
        for _ in 0..12 {
            if edits.is_empty() {
                break;
            }
            let edit = edits.swap_remove(rng.below(edits.len()));
            if check(&format!("generated seed {seed}"), &src, &edit).is_some() {
                checked += 1;
            }
        }
    }
    assert!(checked > 400, "only {checked} edits checked");
}

#[test]
fn large_diagrams_edit_quickly() {
    let mut src = String::from("flowchart LR\n");
    for i in 0..300 {
        src.push_str(&format!(
            "  n{i}[Service {i}] --> n{}[Service {}]\n",
            (i + 1) % 300,
            (i + 1) % 300
        ));
        src.push_str(&format!("  n{i} -.-> n{}\n", (i * 7 + 3) % 300));
    }
    let started = std::time::Instant::now();
    let d = mermaid::parse(&src).unwrap().diagram;
    assert_eq!(d.edges.len(), 600);
    let last = schematlas_parser::model::EdgeRef::of(&d, 599);
    let out = mermaid::apply(&src, &schematlas_parser::model::Edit::RemoveEdge(last)).unwrap();
    assert_eq!(mermaid::parse(&out).unwrap().diagram.edges.len(), 599);
    let elapsed = started.elapsed();
    // Debug builds are slow; this guards against accidental quadratic work, not micro-costs.
    assert!(elapsed.as_secs_f64() < 8.0, "took {elapsed:?}");
    eprintln!("parse + remove last of 600 edges: {elapsed:?}");
}
