//! DBML: read into the relational model, write canonical DBML from a live database, compare a
//! database with the file, and merge database changes into the file without losing the
//! author's notes, comments, groups and annotations.
use schematlas_parser::annotate::Target;
use schematlas_parser::dbml::{self, Cardinality, Change, Column, Endpoint, Ref, Schema, Table};
use schematlas_parser::model::{Annotation, CodeRef};
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dbml")
            .join(name),
    )
    .unwrap()
}
fn table<'a>(s: &'a Schema, name: &str) -> &'a Table {
    s.tables
        .iter()
        .find(|t| t.qualified() == name)
        .unwrap_or_else(|| panic!("no table {name}"))
}
fn column<'a>(t: &'a Table, name: &str) -> &'a Column {
    t.columns.iter().find(|c| c.name == name).unwrap()
}

#[test]
fn reads_tables_columns_settings_and_notes() {
    let s = dbml::parse(&fixture("01-commerce.dbml")).unwrap();
    let project = s.project.as_ref().unwrap();
    assert_eq!(
        (project.name.as_str(), project.database.as_deref()),
        ("commerce", Some("PostgreSQL"))
    );
    assert_eq!(
        project.note.as_deref(),
        Some("Orders, payments and the catalog.")
    );
    assert_eq!(
        s.tables.iter().map(|t| t.qualified()).collect::<Vec<_>>(),
        [
            "public.users",
            "public.orders",
            "core.products",
            "public.order_items"
        ]
    );
    let users = table(&s, "public.users");
    assert_eq!(users.alias.as_deref(), Some("U"));
    assert_eq!(users.note.as_deref(), Some("People who can sign in"));
    let id = column(users, "id");
    assert!(id.primary_key && id.increment);
    let email = column(users, "email");
    assert_eq!(email.data_type, "varchar(255)");
    assert!(email.not_null && email.unique);
    assert_eq!(email.note.as_deref(), Some("Login identifier"));
    assert_eq!(
        column(users, "created_at").default.as_deref(),
        Some("`now()`")
    );
    let orders = table(&s, "public.orders");
    assert_eq!(
        column(orders, "status").default.as_deref(),
        Some("'pending'")
    );
    assert_eq!(column(orders, "total").default.as_deref(), Some("0"));
    assert_eq!(orders.indexes.len(), 2);
    assert_eq!(orders.indexes[0].columns, ["user_id", "status"]);
    assert_eq!(
        orders.indexes[0].name.as_deref(),
        Some("orders_user_status")
    );
    let items = table(&s, "public.order_items");
    assert!(items.indexes[0].primary_key);
    assert_eq!(s.enums[0].name, "order_status");
    assert_eq!(
        s.enums[0].values,
        ["pending", "paid", "shipped", "cancelled"]
    );
    assert_eq!(s.groups[0].name, "sales");
    assert_eq!(s.groups[0].tables, ["public.orders", "public.order_items"]);
}

#[test]
fn references_are_normalised_and_aliases_resolved() {
    let s = dbml::parse(&fixture("01-commerce.dbml")).unwrap();
    let pair = |r: &Ref| (r.from.to_string(), r.to.to_string(), r.cardinality);
    let refs: Vec<_> = s.refs.iter().map(pair).collect();
    assert!(
        refs.contains(&(
            "public.orders.user_id".into(),
            "public.users.id".into(),
            Cardinality::ManyToOne
        )),
        "{refs:?}"
    );
    assert!(refs.contains(&(
        "public.order_items.order_id".into(),
        "public.orders.id".into(),
        Cardinality::ManyToOne
    )));
    assert!(refs.contains(&(
        "public.order_items.product_id".into(),
        "core.products.id".into(),
        Cardinality::ManyToOne
    )));
    let cascade = s.refs.iter().find(|r| r.to.table == "orders").unwrap();
    assert_eq!(cascade.on_delete.as_deref(), Some("cascade"));
    // `a < b` is the same relationship as `b > a`.
    let flipped =
        dbml::parse("Table a { id int }\nTable b { a_id int }\nRef: a.id < b.a_id").unwrap();
    assert_eq!(
        pair(&flipped.refs[0]),
        (
            "public.b.a_id".into(),
            "public.a.id".into(),
            Cardinality::ManyToOne
        )
    );
    let composite = dbml::parse(
        "Table a { x int\n y int }\nTable b { x int\n y int }\nRef: b.(x, y) > a.(x, y)",
    )
    .unwrap();
    assert_eq!(composite.refs[0].from.columns, ["x", "y"]);
}

#[test]
fn annotations_live_in_comments() {
    let s = dbml::parse(&fixture("01-commerce.dbml")).unwrap();
    let orders = table(&s, "public.orders");
    assert_eq!(
        orders.annotation.code,
        [CodeRef::parse("src/orders/model.rs:10-80")]
    );
    assert_eq!(orders.annotation.note.as_deref(), Some("Aggregate root"));
    assert_eq!(s.annotation.title.as_deref(), Some("Commerce data"));
    let annotation = Annotation {
        code: vec![CodeRef::parse("src/users.rs:1")],
        ..Annotation::default()
    };
    let out = dbml::annotate(
        &fixture("01-commerce.dbml"),
        &Target::Node("public.users".into()),
        &annotation,
    )
    .unwrap();
    assert!(
        out.contains("// @schematlas node public.users code=src/users.rs:1\n"),
        "{out}"
    );
    assert_eq!(
        table(&dbml::parse(&out).unwrap(), "public.users").annotation,
        annotation
    );
    assert!(dbml::annotate(&out, &Target::Node("public.nope".into()), &annotation).is_err());
}

#[test]
fn canonical_output_reads_back_identically() {
    for src in [fixture("01-commerce.dbml"), fixture("02-edge-cases.dbml")] {
        let s = dbml::parse(&src).unwrap();
        let written = dbml::write(&s);
        let back = dbml::parse(&written).unwrap_or_else(|e| panic!("{e}\n{written}"));
        assert_eq!(back, s, "\n{written}");
        // Writing is deterministic.
        assert_eq!(dbml::write(&back), written);
    }
}

#[test]
fn diff_names_every_structural_change() {
    let before = dbml::parse(&fixture("01-commerce.dbml")).unwrap();
    let mut after = before.clone();
    after.tables.retain(|t| t.name != "order_items");
    after
        .refs
        .retain(|r| r.from.table != "order_items" && r.to.table != "order_items");
    let orders = after
        .tables
        .iter_mut()
        .find(|t| t.name == "orders")
        .unwrap();
    orders.columns.retain(|c| c.name != "total");
    orders.columns.push(Column {
        name: "currency".into(),
        data_type: "char(3)".into(),
        not_null: true,
        ..Column::default()
    });
    let users = after.tables.iter_mut().find(|t| t.name == "users").unwrap();
    users
        .columns
        .iter_mut()
        .find(|c| c.name == "name")
        .unwrap()
        .data_type = "text".into();
    // Notes are documentation, not structure.
    users.note = Some("Changed note".into());
    after.tables.push(Table {
        schema: "public".into(),
        name: "refunds".into(),
        columns: vec![Column {
            name: "id".into(),
            data_type: "int".into(),
            primary_key: true,
            ..Column::default()
        }],
        ..Table::default()
    });
    let changes = dbml::diff(&before, &after);
    let has = |c: &Change| changes.contains(c);
    assert!(
        has(&Change::TableRemoved("public.order_items".into())),
        "{changes:#?}"
    );
    assert!(has(&Change::TableAdded("public.refunds".into())));
    assert!(has(&Change::ColumnRemoved {
        table: "public.orders".into(),
        column: "total".into()
    }));
    assert!(has(&Change::ColumnAdded {
        table: "public.orders".into(),
        column: "currency".into()
    }));
    assert!(changes
        .iter()
        .any(|c| matches!(c, Change::ColumnChanged { column, .. } if column == "name")));
    assert!(changes.iter().any(|c| matches!(c, Change::RefRemoved(_))));
    assert!(
        !changes
            .iter()
            .any(|c| format!("{c:?}").contains("Changed note")),
        "{changes:#?}"
    );
    assert!(dbml::diff(&before, &before).is_empty());
}

#[test]
fn merging_a_database_keeps_the_authors_words() {
    let src = fixture("01-commerce.dbml");
    let before = dbml::parse(&src).unwrap();
    // What a live database would report: structure only, no notes or groups.
    let mut live = dbml::structure(&before);
    live.tables.retain(|t| t.name != "order_items");
    live.refs
        .retain(|r| r.from.table != "order_items" && r.to.table != "order_items");
    let orders = live.tables.iter_mut().find(|t| t.name == "orders").unwrap();
    orders.columns.push(Column {
        name: "currency".into(),
        data_type: "char(3)".into(),
        not_null: true,
        default: Some("'EUR'".into()),
        ..Column::default()
    });
    live.tables
        .iter_mut()
        .find(|t| t.name == "users")
        .unwrap()
        .columns
        .retain(|c| c.name != "name");
    live.tables.push(Table {
        schema: "public".into(),
        name: "refunds".into(),
        columns: vec![
            Column {
                name: "id".into(),
                data_type: "integer".into(),
                primary_key: true,
                ..Column::default()
            },
            Column {
                name: "order_id".into(),
                data_type: "integer".into(),
                ..Column::default()
            },
        ],
        ..Table::default()
    });
    live.refs.push(Ref {
        name: None,
        from: Endpoint::new("public", "refunds", &["order_id"]),
        to: Endpoint::new("public", "orders", &["id"]),
        cardinality: Cardinality::ManyToOne,
        on_delete: None,
        on_update: None,
    });

    let merged = dbml::merge(&src, &live).unwrap();
    let after = dbml::parse(&merged).unwrap_or_else(|e| panic!("{e}\n{merged}"));
    assert_eq!(
        dbml::structure(&after),
        dbml::structure(&live),
        "\n{merged}"
    );
    // Words written by people survive.
    assert!(
        merged.starts_with(
            "// Commerce schema, maintained by hand and by Schematlas.\nProject commerce {"
        ),
        "{merged}"
    );
    assert!(merged.contains("// Orders reference users inline."));
    assert_eq!(
        table(&after, "public.users").note.as_deref(),
        Some("People who can sign in")
    );
    assert_eq!(
        column(table(&after, "public.users"), "email")
            .note
            .as_deref(),
        Some("Login identifier")
    );
    assert_eq!(
        table(&after, "public.orders").annotation.note.as_deref(),
        Some("Aggregate root")
    );
    assert_eq!(after.annotation.title.as_deref(), Some("Commerce data"));
    // The removed table leaves its group; the untouched inline reference keeps its form.
    assert_eq!(after.groups[0].tables, ["public.orders"]);
    assert!(
        merged.contains("user_id integer [not null, ref: > U.id]"),
        "{merged}"
    );
    // Nothing to do is nothing changed.
    assert_eq!(dbml::merge(&src, &dbml::structure(&before)).unwrap(), src);
}

#[test]
fn invalid_files_are_errors_with_positions() {
    let err = dbml::parse("Table users {\n  id int [pk\n}").unwrap_err();
    assert!(
        matches!(
            err,
            schematlas_parser::Error::Parse {
                offset: Some(_),
                ..
            }
        ),
        "{err:?}"
    );
    assert!(
        dbml::parse("Ref: a.id > b.id").is_err(),
        "references must point at tables"
    );
}
