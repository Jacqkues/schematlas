//! Random schemas evolve like real databases; merging each new state into the file must give the
//! exact structure while keeping every note, comment and annotation that still has a home.
mod support;
use schematlas_parser::dbml::{
    self, Cardinality, Column, Endpoint, Enum, Index, Ref, Schema, Table, TableGroup,
};
use schematlas_parser::model::{Annotation, CodeRef};
use support::Rng;

const TYPES: [&str; 7] = [
    "integer",
    "bigint",
    "varchar(255)",
    "text",
    "timestamp",
    "decimal(10,2)",
    "boolean",
];

fn column(rng: &mut Rng, name: String, pk: bool) -> Column {
    Column {
        name,
        data_type: TYPES[rng.below(TYPES.len())].into(),
        primary_key: pk,
        unique: !pk && rng.below(5) == 0,
        not_null: rng.below(2) == 0,
        increment: pk && rng.below(2) == 0,
        default: match rng.below(6) {
            0 => Some("0".into()),
            1 => Some("'draft'".into()),
            2 => Some("`now()`".into()),
            _ => None,
        },
        note: None,
    }
}

fn generate(seed: u64) -> Schema {
    let mut rng = Rng(seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1);
    let mut s = Schema::default();
    for t in 0..2 + rng.below(6) {
        let schema = if rng.below(4) == 0 { "audit" } else { "public" };
        let mut columns = vec![column(&mut rng, "id".into(), true)];
        for c in 0..1 + rng.below(5) {
            columns.push(column(&mut rng, format!("c{c}"), false));
        }
        let indexes = if rng.below(3) == 0 && columns.len() > 2 {
            vec![Index {
                name: Some(format!("idx_t{t}")),
                columns: vec![columns[1].name.clone(), columns[2].name.clone()],
                unique: rng.below(2) == 0,
                primary_key: false,
            }]
        } else {
            vec![]
        };
        s.tables.push(Table {
            schema: schema.into(),
            name: format!("t{t}"),
            note: (rng.below(2) == 0).then(|| format!("Table {t} explained")),
            columns: columns
                .into_iter()
                .map(|mut c| {
                    if rng.below(3) == 0 {
                        c.note = Some(format!("about {}", c.name));
                    }
                    c
                })
                .collect(),
            indexes,
            annotation: if rng.below(3) == 0 {
                Annotation {
                    code: vec![CodeRef::parse(&format!("src/t{t}.rs:1-9"))],
                    ..Annotation::default()
                }
            } else {
                Annotation::default()
            },
            ..Table::default()
        });
    }
    for _ in 0..rng.below(4) {
        let (a, b) = (rng.below(s.tables.len()), rng.below(s.tables.len()));
        if a == b {
            continue;
        }
        let from = &s.tables[a];
        let col = &from.columns[rng.below(from.columns.len())];
        let to = &s.tables[b];
        let r = Ref {
            name: None,
            from: Endpoint::new(&from.schema, &from.name, &[&col.name]),
            to: Endpoint::new(&to.schema, &to.name, &["id"]),
            cardinality: Cardinality::ManyToOne,
            on_delete: (rng.below(3) == 0).then(|| "cascade".into()),
            on_update: None,
        };
        if !s.refs.contains(&r) {
            s.refs.push(r);
        }
    }
    if rng.below(2) == 0 {
        s.enums.push(Enum {
            schema: "public".into(),
            name: "state".into(),
            values: vec!["draft".into(), "live".into()],
        });
    }
    if s.tables.len() > 1 {
        s.groups.push(TableGroup {
            name: "core".into(),
            tables: vec![s.tables[0].qualified(), s.tables[1].qualified()],
        });
    }
    s
}

/// One evolution step a database might take.
fn evolve(live: &mut Schema, rng: &mut Rng, step: usize) {
    match rng.below(8) {
        0 if live.tables.len() > 1 => {
            let gone = live.tables.remove(rng.below(live.tables.len())).qualified();
            live.refs
                .retain(|r| r.from.qualified_table() != gone && r.to.qualified_table() != gone);
        }
        1 => live.tables.push(Table {
            name: format!("new{step}"),
            columns: vec![
                column(rng, "id".into(), true),
                column(rng, "body".into(), false),
            ],
            ..Table::default()
        }),
        2 => {
            let i = rng.below(live.tables.len());
            let c = column(rng, format!("added{step}"), false);
            live.tables[i].columns.push(c);
        }
        3 => {
            let i = rng.below(live.tables.len());
            let t = &mut live.tables[i];
            if t.columns.len() > 1 {
                let gone = t.columns.remove(1 + rng.below(t.columns.len() - 1)).name;
                let q = t.qualified();
                live.refs.retain(|r| {
                    !(r.from.qualified_table() == q && r.from.columns.contains(&gone)
                        || r.to.qualified_table() == q && r.to.columns.contains(&gone))
                });
                t.indexes.retain(|ix| !ix.columns.contains(&gone));
            }
        }
        4 => {
            let i = rng.below(live.tables.len());
            let t = &mut live.tables[i];
            let j = rng.below(t.columns.len());
            t.columns[j].not_null = !t.columns[j].not_null;
            t.columns[j].data_type = TYPES[rng.below(TYPES.len())].into();
        }
        5 if !live.refs.is_empty() => {
            live.refs.remove(rng.below(live.refs.len()));
        }
        6 if live.tables.len() > 1 => {
            let (a, b) = (0, live.tables.len() - 1);
            let r = Ref {
                name: None,
                from: Endpoint::new(&live.tables[b].schema, &live.tables[b].name, &["id"]),
                to: Endpoint::new(&live.tables[a].schema, &live.tables[a].name, &["id"]),
                cardinality: Cardinality::OneToOne,
                on_delete: None,
                on_update: None,
            };
            let r = dbml::parse(&dbml::write(&Schema {
                tables: live.tables.clone(),
                refs: vec![r],
                ..Schema::default()
            }))
            .unwrap()
            .refs
            .remove(0);
            if !live.refs.contains(&r) {
                live.refs.push(r);
            }
        }
        _ => match live.enums.first_mut() {
            Some(e) => e.values.push(format!("v{step}")),
            None => live.enums.push(Enum {
                schema: "public".into(),
                name: "kind".into(),
                values: vec!["a".into()],
            }),
        },
    }
}

#[test]
fn files_follow_their_database_and_keep_their_words() {
    let seeds: u64 = std::env::var("SCHEMATLAS_GEN_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let mut merges = 0;
    for seed in 1..=seeds {
        let original = generate(seed);
        // Comments between and inside blocks, where people put them.
        let mut sprinkle = Rng(seed ^ 0xABCD);
        let body: String = dbml::write(&original)
            .lines()
            .map(|line| {
                let comment = match sprinkle.below(6) {
                    0 if line.starts_with("  ") && !line.trim_start().starts_with('(') => {
                        "  // about the line above\n"
                    }
                    1 if line.is_empty() => "// a comment between blocks\n",
                    _ => "",
                };
                format!("{line}\n{comment}")
            })
            .collect();
        let mut text = format!("// Seed {seed}: hand-written header\n{body}");
        let parsed = dbml::parse(&text).unwrap_or_else(|e| panic!("seed {seed}: {e}\n{text}"));
        assert_eq!(parsed.tables, original.tables, "seed {seed}\n{text}");
        // An unchanged database changes nothing.
        assert_eq!(
            dbml::merge(&text, &dbml::structure(&parsed)).unwrap(),
            text,
            "seed {seed}"
        );
        let mut live = dbml::structure(&parsed);
        let mut rng = Rng(seed + 99);
        for step in 0..8 {
            evolve(&mut live, &mut rng, step);
            let before = dbml::parse(&text).unwrap();
            let merged = dbml::merge(&text, &live)
                .unwrap_or_else(|e| panic!("seed {seed} step {step}: {e}\n{text}"));
            let after = dbml::parse(&merged).unwrap();
            assert!(
                dbml::diff(&after, &live).is_empty(),
                "seed {seed} step {step}: {:#?}\n{merged}",
                dbml::diff(&after, &live)
            );
            assert!(merged.starts_with(&format!("// Seed {seed}: hand-written header\n")));
            // Every note and annotation whose table or column survived is still there.
            for t in &before.tables {
                let Some(now) = after.tables.iter().find(|x| x.qualified() == t.qualified()) else {
                    continue;
                };
                assert_eq!(
                    now.note,
                    t.note,
                    "seed {seed} step {step}: {}",
                    t.qualified()
                );
                assert_eq!(
                    now.annotation,
                    t.annotation,
                    "seed {seed} step {step}: {}",
                    t.qualified()
                );
                for c in &t.columns {
                    if let Some(nc) = now.columns.iter().find(|x| x.name == c.name) {
                        assert_eq!(
                            nc.note,
                            c.note,
                            "seed {seed} step {step}: {}.{}",
                            t.qualified(),
                            c.name
                        );
                    }
                }
            }
            text = merged;
            merges += 1;
        }
    }
    assert!(merges >= 400, "{merges}");
}
