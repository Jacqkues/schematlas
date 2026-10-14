//! DBML: database schemas as text in the repository.
//!
//! [`parse`] reads a file (with `dbml-rs`, resolving aliases and the default schema here);
//! [`write`] produces canonical DBML from a live database's schema; [`diff`] lists structural
//! differences; [`merge`] applies a live schema to an existing file element by element, so the
//! notes, comments, groups and `// @schematlas` annotations people wrote stay where they were.
use crate::annotate::{self, Target};
use crate::error::{Error, Result};
use crate::model::Annotation;
use dbml_rs::ast::{self, TopLevelBlock};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::ops::Range;

pub const DEFAULT_SCHEMA: &str = "public";
const COMMENT: &str = "//";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<Project>,
    pub tables: Vec<Table>,
    pub refs: Vec<Ref>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enums: Vec<Enum>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<TableGroup>,
    #[serde(skip_serializing_if = "Annotation::is_empty")]
    pub annotation: Annotation,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub schema: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub columns: Vec<Column>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub indexes: Vec<Index>,
    #[serde(skip_serializing_if = "Annotation::is_empty")]
    pub annotation: Annotation,
}
impl Default for Table {
    fn default() -> Self {
        Table {
            schema: DEFAULT_SCHEMA.into(),
            name: String::new(),
            alias: None,
            note: None,
            columns: vec![],
            indexes: vec![],
            annotation: Annotation::default(),
        }
    }
}
impl Table {
    /// `schema.name`, the table's identity everywhere in this module.
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub data_type: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub primary_key: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unique: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub not_null: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub increment: bool,
    /// The DBML literal as written: `'pending'`, `0`, `` `now()` ``, `true`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Column names, or `` `expression` `` for expression indexes.
    pub columns: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unique: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub primary_key: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Endpoint {
    pub schema: String,
    pub table: String,
    pub columns: Vec<String>,
}
impl Endpoint {
    pub fn new(schema: &str, table: &str, columns: &[&str]) -> Self {
        Endpoint {
            schema: schema.into(),
            table: table.into(),
            columns: columns.iter().map(|c| (*c).to_owned()).collect(),
        }
    }
    pub fn qualified_table(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}
impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.columns.as_slice() {
            [one] => write!(f, "{}.{}.{one}", self.schema, self.table),
            many => write!(f, "{}.{}.({})", self.schema, self.table, many.join(", ")),
        }
    }
}
/// Relationships are normalised: `a < b` is stored as `b > a`, and symmetric kinds order their
/// endpoints, so the same relationship always compares equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    ManyToOne,
    OneToOne,
    ManyToMany,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ref {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub from: Endpoint,
    pub to: Endpoint,
    pub cardinality: Cardinality,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_delete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_update: Option<String>,
}
impl Ref {
    fn normalized(mut self) -> Self {
        if self.cardinality != Cardinality::ManyToOne && self.to < self.from {
            std::mem::swap(&mut self.from, &mut self.to);
        }
        self
    }
    /// The relationship without its name, for comparisons.
    fn shape(
        &self,
    ) -> (
        Endpoint,
        Endpoint,
        Cardinality,
        Option<String>,
        Option<String>,
    ) {
        (
            self.from.clone(),
            self.to.clone(),
            self.cardinality,
            self.on_delete.clone(),
            self.on_update.clone(),
        )
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Enum {
    pub schema: String,
    pub name: String,
    pub values: Vec<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TableGroup {
    pub name: String,
    /// Qualified table names.
    pub tables: Vec<String>,
}

// ---------- Reading ----------

/// Where things are in the source, for [`merge`].
#[derive(Default)]
struct Spans {
    blocks: Vec<Range<usize>>,
    tables: HashMap<String, TableSpans>,
    /// Top-level `Ref` blocks with the relationship they declare.
    ref_blocks: Vec<(Range<usize>, Ref)>,
    enums: HashMap<String, Range<usize>>,
    group_items: Vec<(Range<usize>, String, usize)>,
    groups: Vec<Range<usize>>,
    annotations: Vec<(Range<usize>, Target)>,
}
#[derive(Default)]
struct TableSpans {
    block: Range<usize>,
    columns: HashMap<String, Range<usize>>,
    /// Inline refs per column, normalised.
    inline: HashMap<String, Vec<Ref>>,
    column_indent: String,
    indexes: Option<Range<usize>>,
    /// Where new columns go: after the last column.
    columns_end: usize,
}

fn note_text(note: &Option<ast::NoteBlock>) -> Option<String> {
    note.as_ref().map(|n| literal_text(&n.value))
}
fn literal_text(l: &ast::Literal) -> String {
    match &l.value {
        ast::Value::String(s) => s.clone(),
        _ => l.raw.clone(),
    }
}
fn action(a: &Option<ast::ReferentialAction>) -> Option<String> {
    a.as_ref().map(|a| {
        match a {
            ast::ReferentialAction::NoAction => "no action",
            ast::ReferentialAction::Cascade => "cascade",
            ast::ReferentialAction::Restrict => "restrict",
            ast::ReferentialAction::SetNull => "set null",
            ast::ReferentialAction::SetDefault => "set default",
        }
        .to_owned()
    })
}

fn pest_error(e: pest::error::Error<impl pest::RuleType>, src: &str) -> Error {
    let offset = match e.location {
        pest::error::InputLocation::Pos(p) => p,
        pest::error::InputLocation::Span((s, _)) => s,
    };
    let (line, col) = match e.line_col {
        pest::error::LineColLocation::Pos(p) | pest::error::LineColLocation::Span(p, _) => p,
    };
    let _ = src;
    Error::Parse {
        message: format!("DBML line {line}, column {col}: {}", e.variant.message()),
        offset: Some(offset),
    }
}

struct Resolver {
    aliases: HashMap<String, (String, String)>,
    tables: Vec<(String, String)>,
}
impl Resolver {
    fn table(&self, schema: Option<&ast::Ident>, name: &ast::Ident) -> Result<(String, String)> {
        let found = match schema {
            Some(s) => Some((s.to_string.clone(), name.to_string.clone())),
            None => self
                .aliases
                .get(&name.to_string)
                .cloned()
                .or_else(|| Some((DEFAULT_SCHEMA.to_owned(), name.to_string.clone()))),
        };
        match found {
            Some(t) if self.tables.contains(&t) => Ok(t),
            _ => Err(Error::Parse {
                message: format!("reference to unknown table `{}`", name.to_string),
                offset: Some(name.span_range.start),
            }),
        }
    }
    fn endpoint(&self, r: &ast::RefIdent) -> Result<Endpoint> {
        let (schema, table) = self.table(r.schema.as_ref(), &r.table)?;
        Ok(Endpoint {
            schema,
            table,
            columns: r.compositions.iter().map(|c| c.to_string.clone()).collect(),
        })
    }
}
fn relation(
    lhs: Endpoint,
    rel: &ast::Relation,
    rhs: Endpoint,
    name: Option<String>,
    settings: Option<&ast::RefSettings>,
) -> Result<Ref> {
    let (from, to, cardinality) = match rel {
        ast::Relation::Many2One => (lhs, rhs, Cardinality::ManyToOne),
        ast::Relation::One2Many => (rhs, lhs, Cardinality::ManyToOne),
        ast::Relation::One2One => (lhs, rhs, Cardinality::OneToOne),
        ast::Relation::Many2Many => (lhs, rhs, Cardinality::ManyToMany),
        ast::Relation::Undef => {
            return Err(Error::Parse {
                message: "reference without a relation".into(),
                offset: None,
            })
        }
    };
    Ok(Ref {
        name,
        from,
        to,
        cardinality,
        on_delete: settings.and_then(|s| action(&s.on_delete)),
        on_update: settings.and_then(|s| action(&s.on_update)),
    }
    .normalized())
}

/// A span without surrounding whitespace or trailing whole-line comments: the parser's spans run
/// on through whatever it skipped after an element, including people's comments below it.
fn trimmed(src: &str, range: &Range<usize>) -> Range<usize> {
    let text = &src[range.clone()];
    let start = range.start + (text.len() - text.trim_start().len());
    let mut end = range.start + text.trim_end().len();
    while end > start {
        let line_start = src[..end].rfind('\n').map_or(0, |p| p + 1).max(start);
        if line_start > start && src[line_start..end].trim_start().starts_with("//") {
            end = line_start + src[line_start..end].len() - src[line_start..end].trim_start().len();
            end = start + src[start..end].trim_end().len();
        } else {
            break;
        }
    }
    start..end.max(start)
}
fn indent_of(src: &str, offset: usize) -> String {
    let line_start = src[..offset].rfind('\n').map_or(0, |p| p + 1);
    src[line_start..offset]
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

fn read(src: &str) -> Result<(Schema, Spans)> {
    let ast = dbml_rs::parse_dbml_unchecked(src).map_err(|e| pest_error(e, src))?;
    let mut spans = Spans::default();
    let mut schema = Schema::default();
    let resolver = Resolver {
        aliases: ast
            .tables()
            .iter()
            .filter_map(|t| {
                let alias = t.ident.alias.as_ref()?;
                let s = t
                    .ident
                    .schema
                    .as_ref()
                    .map_or(DEFAULT_SCHEMA.to_owned(), |s| s.to_string.clone());
                Some((alias.to_string.clone(), (s, t.ident.name.to_string.clone())))
            })
            .collect(),
        tables: ast
            .tables()
            .iter()
            .map(|t| {
                (
                    t.ident
                        .schema
                        .as_ref()
                        .map_or(DEFAULT_SCHEMA.to_owned(), |s| s.to_string.clone()),
                    t.ident.name.to_string.clone(),
                )
            })
            .collect(),
    };
    for block in &ast.blocks {
        match block {
            TopLevelBlock::Project(p) => {
                spans.blocks.push(p.span_range.clone());
                schema.project = Some(Project {
                    name: p.ident.to_string.clone(),
                    database: p
                        .properties
                        .iter()
                        .find(|x| x.key.to_string == "database_type")
                        .map(|x| literal_text(&x.value)),
                    note: note_text(&p.note),
                });
            }
            TopLevelBlock::Enum(e) => {
                spans.blocks.push(e.span_range.clone());
                let s = e
                    .ident
                    .schema
                    .as_ref()
                    .map_or(DEFAULT_SCHEMA.to_owned(), |s| s.to_string.clone());
                let en = Enum {
                    schema: s,
                    name: e.ident.name.to_string.clone(),
                    values: e.values.iter().map(|v| v.value.to_string.clone()).collect(),
                };
                spans
                    .enums
                    .insert(format!("{}.{}", en.schema, en.name), e.span_range.clone());
                schema.enums.push(en);
            }
            TopLevelBlock::Table(t) => {
                spans.blocks.push(t.span_range.clone());
                let s = t
                    .ident
                    .schema
                    .as_ref()
                    .map_or(DEFAULT_SCHEMA.to_owned(), |s| s.to_string.clone());
                let mut table = Table {
                    schema: s,
                    name: t.ident.name.to_string.clone(),
                    alias: t.ident.alias.as_ref().map(|a| a.to_string.clone()),
                    note: note_text(&t.note),
                    ..Table::default()
                };
                let mut ts = TableSpans {
                    block: trimmed(src, &t.span_range),
                    ..TableSpans::default()
                };
                for c in &t.cols {
                    let settings = c.settings.as_ref();
                    let raw = |key: &str| {
                        settings
                            .and_then(|s| s.attributes.iter().find(|a| a.key.to_string == key))
                            .and_then(|a| a.value.as_ref())
                            .map(|v| v.raw.clone())
                    };
                    let column = Column {
                        name: c.name.to_string.clone(),
                        data_type: c.r#type.raw.trim().to_owned(),
                        primary_key: settings.is_some_and(|s| s.is_pk),
                        unique: settings.is_some_and(|s| s.is_unique),
                        not_null: settings
                            .is_some_and(|s| s.nullable == Some(ast::Nullable::NotNull)),
                        increment: settings.is_some_and(|s| s.is_incremental),
                        default: raw("default"),
                        note: settings.and_then(|s| s.note.clone()),
                    };
                    let range = trimmed(src, &c.span_range);
                    if ts.column_indent.is_empty() {
                        ts.column_indent = indent_of(src, range.start);
                    }
                    ts.columns_end = ts.columns_end.max(range.end);
                    ts.columns.insert(column.name.clone(), range);
                    let lhs = Endpoint {
                        schema: table.schema.clone(),
                        table: table.name.clone(),
                        columns: vec![column.name.clone()],
                    };
                    let mut inline = vec![];
                    for r in settings.map(|s| s.refs.as_slice()).unwrap_or_default() {
                        let rhs = resolver.endpoint(&r.rhs)?;
                        let rel = relation(lhs.clone(), &r.rel, rhs, None, None)?;
                        inline.push(rel.clone());
                        schema.refs.push(rel);
                    }
                    ts.inline.insert(column.name.clone(), inline);
                    table.columns.push(column);
                }
                if let Some(indexes) = &t.indexes {
                    // The span starts at the braces; the change must also cover the keyword.
                    let range = trimmed(src, &indexes.span_range);
                    let start = src[..range.start]
                        .trim_end()
                        .strip_suffix("indexes")
                        .map_or(range.start, str::len);
                    ts.indexes = Some(start..range.end);
                    for def in &indexes.defs {
                        let s = def.settings.as_ref();
                        table.indexes.push(Index {
                            name: s.and_then(|s| s.name.clone()),
                            columns: def
                                .cols
                                .iter()
                                .map(|c| match c {
                                    ast::IndexesColumnType::String(i) => i.to_string.clone(),
                                    ast::IndexesColumnType::Expr(l) => l.raw.clone(),
                                })
                                .collect(),
                            unique: s.is_some_and(|s| s.is_unique),
                            primary_key: s.is_some_and(|s| s.is_pk),
                        });
                    }
                }
                if ts.column_indent.is_empty() {
                    ts.column_indent = "  ".into();
                }
                spans.tables.insert(table.qualified(), ts);
                schema.tables.push(table);
            }
            TopLevelBlock::Ref(r) => {
                spans.blocks.push(r.span_range.clone());
                let rel = relation(
                    resolver.endpoint(&r.lhs)?,
                    &r.rel,
                    resolver.endpoint(&r.rhs)?,
                    r.name.as_ref().map(|n| n.to_string.clone()),
                    r.settings.as_ref(),
                )?;
                spans
                    .ref_blocks
                    .push((trimmed(src, &r.span_range), rel.clone()));
                schema.refs.push(rel);
            }
            TopLevelBlock::TableGroup(g) => {
                spans.blocks.push(g.span_range.clone());
                let index = schema.groups.len();
                spans.groups.push(trimmed(src, &g.span_range));
                let mut tables = vec![];
                for item in &g.items {
                    let (s, t) = resolver.table(item.schema.as_ref(), &item.ident_alias)?;
                    let name = format!("{s}.{t}");
                    spans
                        .group_items
                        .push((trimmed(src, &item.span_range), name.clone(), index));
                    tables.push(name);
                }
                schema.groups.push(TableGroup {
                    name: g.ident.to_string.clone(),
                    tables,
                });
            }
            TopLevelBlock::Note(n) => spans.blocks.push(n.span_range.clone()),
        }
    }
    // Annotations in `// @schematlas` comment lines.
    let mut offset = 0;
    for line in src.split_inclusive('\n') {
        if let Some((target, annotation)) = annotate::parse_comment(line, COMMENT) {
            match &target {
                Target::Diagram => schema.annotation = annotation,
                Target::Node(name) => {
                    if let Some(t) = schema.tables.iter_mut().find(|t| &t.qualified() == name) {
                        t.annotation = annotation;
                    }
                }
                Target::Edge(_) => {}
            }
            spans
                .annotations
                .push((offset..offset + line.len(), target));
        }
        offset += line.len();
    }
    dedupe_refs(&mut schema.refs);
    Ok((schema, spans))
}

fn dedupe_refs(refs: &mut Vec<Ref>) {
    let mut seen = vec![];
    refs.retain(|r| {
        let key = r.shape();
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
}

/// Read a DBML file.
pub fn parse(src: &str) -> Result<Schema> {
    read(src).map(|(schema, _)| schema)
}

/// The schema as a database reports it: structure only, without notes, names chosen by people,
/// groups, the project block or annotations.
pub fn structure(schema: &Schema) -> Schema {
    let mut s = schema.clone();
    s.project = None;
    s.groups.clear();
    s.annotation = Annotation::default();
    for t in &mut s.tables {
        t.note = None;
        t.alias = None;
        t.annotation = Annotation::default();
        for c in &mut t.columns {
            c.note = None;
        }
        for i in &mut t.indexes {
            i.name = None;
        }
    }
    for r in &mut s.refs {
        r.name = None;
    }
    s
}

// ---------- Writing ----------

fn ident(name: &str) -> String {
    let plain = name
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if plain {
        name.to_owned()
    } else {
        format!("\"{}\"", name.replace('"', "\\\""))
    }
}
fn qualified_ident(schema: &str, name: &str) -> String {
    if schema == DEFAULT_SCHEMA {
        ident(name)
    } else {
        format!("{}.{}", ident(schema), ident(name))
    }
}
fn string_literal(text: &str) -> String {
    if text.contains('\n') {
        format!("'''{}'''", text.replace("'''", "\\'''"))
    } else {
        format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}
fn endpoint_text(e: &Endpoint) -> String {
    let table = qualified_ident(&e.schema, &e.table);
    match e.columns.as_slice() {
        [one] => format!("{table}.{}", ident(one)),
        many => format!(
            "{table}.({})",
            many.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ")
        ),
    }
}
fn symbol(c: Cardinality) -> &'static str {
    match c {
        Cardinality::ManyToOne => ">",
        Cardinality::OneToOne => "-",
        Cardinality::ManyToMany => "<>",
    }
}
fn column_line(c: &Column, inline: &[(String, String)]) -> String {
    let mut settings: Vec<String> = vec![];
    if c.primary_key {
        settings.push("pk".into());
    }
    if c.increment {
        settings.push("increment".into());
    }
    if c.not_null {
        settings.push("not null".into());
    }
    if c.unique {
        settings.push("unique".into());
    }
    if let Some(d) = &c.default {
        // dbml-rs cannot read negative number literals; an expression says the same thing.
        let negative = d.starts_with('-') && d[1..].parse::<f64>().is_ok();
        settings.push(if negative {
            format!("default: `{d}`")
        } else {
            format!("default: {d}")
        });
    }
    for (sym, target) in inline {
        settings.push(format!("ref: {sym} {target}"));
    }
    if let Some(note) = &c.note {
        settings.push(format!("note: {}", string_literal(note)));
    }
    let mut line = format!("{} {}", ident(&c.name), c.data_type);
    if !settings.is_empty() {
        line.push_str(&format!(" [{}]", settings.join(", ")));
    }
    line
}
fn index_line(i: &Index) -> String {
    let cols: Vec<String> = i
        .columns
        .iter()
        .map(|c| {
            if c.starts_with('`') {
                c.clone()
            } else {
                ident(c)
            }
        })
        .collect();
    let mut line = if cols.len() == 1 {
        cols[0].clone()
    } else {
        format!("({})", cols.join(", "))
    };
    let mut settings = vec![];
    if i.primary_key {
        settings.push("pk".to_owned());
    }
    if i.unique {
        settings.push("unique".to_owned());
    }
    if let Some(name) = &i.name {
        settings.push(format!("name: {}", string_literal(name)));
    }
    if !settings.is_empty() {
        line.push_str(&format!(" [{}]", settings.join(", ")));
    }
    line
}
fn indexes_block(indexes: &[Index], indent: &str) -> String {
    let mut out = format!("{indent}indexes {{\n");
    for i in indexes {
        out.push_str(&format!("{indent}{indent}{}\n", index_line(i)));
    }
    out.push_str(&format!("{indent}}}"));
    out
}
fn table_block(t: &Table) -> String {
    let mut out = format!("Table {}", qualified_ident(&t.schema, &t.name));
    if let Some(alias) = &t.alias {
        out.push_str(&format!(" as {}", ident(alias)));
    }
    out.push_str(" {\n");
    for c in &t.columns {
        out.push_str(&format!("  {}\n", column_line(c, &[])));
    }
    if !t.indexes.is_empty() {
        out.push('\n');
        out.push_str(&indexes_block(&t.indexes, "  "));
        out.push('\n');
    }
    if let Some(note) = &t.note {
        out.push_str(&format!("\n  Note: {}\n", string_literal(note)));
    }
    out.push('}');
    out
}
fn ref_line(r: &Ref) -> String {
    let mut line = String::from("Ref");
    if let Some(name) = &r.name {
        line.push(' ');
        line.push_str(&ident(name));
    }
    line.push_str(&format!(
        ": {} {} {}",
        endpoint_text(&r.from),
        symbol(r.cardinality),
        endpoint_text(&r.to)
    ));
    let mut settings = vec![];
    if let Some(a) = &r.on_delete {
        settings.push(format!("delete: {a}"));
    }
    if let Some(a) = &r.on_update {
        settings.push(format!("update: {a}"));
    }
    if !settings.is_empty() {
        line.push_str(&format!(" [{}]", settings.join(", ")));
    }
    line
}
fn enum_block(e: &Enum) -> String {
    let mut out = format!("enum {} {{\n", qualified_ident(&e.schema, &e.name));
    for v in &e.values {
        out.push_str(&format!("  {}\n", ident(v)));
    }
    out.push('}');
    out
}

/// Canonical DBML for a schema, e.g. one read from a live database.
pub fn write(schema: &Schema) -> String {
    let mut parts: Vec<String> = vec![];
    if let Some(p) = &schema.project {
        let mut block = format!("Project {} {{\n", ident(&p.name));
        if let Some(db) = &p.database {
            block.push_str(&format!("  database_type: {}\n", string_literal(db)));
        }
        if let Some(note) = &p.note {
            block.push_str(&format!("  Note: {}\n", string_literal(note)));
        }
        block.push('}');
        parts.push(block);
    }
    parts.extend(schema.enums.iter().map(enum_block));
    parts.extend(schema.tables.iter().map(table_block));
    if !schema.refs.is_empty() {
        parts.push(
            schema
                .refs
                .iter()
                .map(ref_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    for g in &schema.groups {
        let mut block = format!("TableGroup {} {{\n", ident(&g.name));
        for t in &g.tables {
            let (s, n) = t.split_once('.').unwrap_or((DEFAULT_SCHEMA, t));
            block.push_str(&format!("  {}\n", qualified_ident(s, n)));
        }
        block.push('}');
        parts.push(block);
    }
    let mut notes = vec![];
    if !schema.annotation.is_empty() {
        notes.push(annotate::format_comment(
            COMMENT,
            &Target::Diagram,
            &schema.annotation,
        ));
    }
    for t in &schema.tables {
        if !t.annotation.is_empty() {
            notes.push(annotate::format_comment(
                COMMENT,
                &Target::Node(t.qualified()),
                &t.annotation,
            ));
        }
    }
    if !notes.is_empty() {
        parts.push(notes.join("\n"));
    }
    let mut out = parts.join("\n\n");
    out.push('\n');
    out
}

// ---------- Comparing ----------

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Change {
    TableAdded(String),
    TableRemoved(String),
    ColumnAdded {
        table: String,
        column: String,
    },
    ColumnRemoved {
        table: String,
        column: String,
    },
    ColumnChanged {
        table: String,
        column: String,
        before: Column,
        after: Column,
    },
    IndexesChanged(String),
    RefAdded(Ref),
    RefRemoved(Ref),
    EnumAdded(String),
    EnumRemoved(String),
    EnumChanged(String),
}

/// Structural differences from `before` to `after`; notes, names and groups are ignored.
pub fn diff(before: &Schema, after: &Schema) -> Vec<Change> {
    let (before, after) = (structure(before), structure(after));
    let mut changes = vec![];
    for t in &before.tables {
        let Some(n) = after.tables.iter().find(|x| x.qualified() == t.qualified()) else {
            changes.push(Change::TableRemoved(t.qualified()));
            continue;
        };
        for c in &t.columns {
            match n.columns.iter().find(|x| x.name == c.name) {
                None => changes.push(Change::ColumnRemoved {
                    table: t.qualified(),
                    column: c.name.clone(),
                }),
                Some(nc) if nc != c => changes.push(Change::ColumnChanged {
                    table: t.qualified(),
                    column: c.name.clone(),
                    before: c.clone(),
                    after: nc.clone(),
                }),
                _ => {}
            }
        }
        for c in &n.columns {
            if !t.columns.iter().any(|x| x.name == c.name) {
                changes.push(Change::ColumnAdded {
                    table: t.qualified(),
                    column: c.name.clone(),
                });
            }
        }
        if t.indexes != n.indexes {
            changes.push(Change::IndexesChanged(t.qualified()));
        }
    }
    for t in &after.tables {
        if !before.tables.iter().any(|x| x.qualified() == t.qualified()) {
            changes.push(Change::TableAdded(t.qualified()));
        }
    }
    for r in &before.refs {
        if !after.refs.iter().any(|x| x.shape() == r.shape()) {
            changes.push(Change::RefRemoved(r.clone()));
        }
    }
    for r in &after.refs {
        if !before.refs.iter().any(|x| x.shape() == r.shape()) {
            changes.push(Change::RefAdded(r.clone()));
        }
    }
    let key = |e: &Enum| format!("{}.{}", e.schema, e.name);
    for e in &before.enums {
        match after.enums.iter().find(|x| key(x) == key(e)) {
            None => changes.push(Change::EnumRemoved(key(e))),
            Some(x) if x.values != e.values => changes.push(Change::EnumChanged(key(e))),
            _ => {}
        }
    }
    for e in &after.enums {
        if !before.enums.iter().any(|x| key(x) == key(e)) {
            changes.push(Change::EnumAdded(key(e)));
        }
    }
    changes
}

// ---------- Merging ----------

struct Patches<'a> {
    src: &'a str,
    list: Vec<(Range<usize>, String)>,
}
impl Patches<'_> {
    /// Remove a range, taking its whole line when nothing else is on it.
    fn remove(&mut self, range: Range<usize>) {
        let src = self.src;
        let line_start = src[..range.start].rfind('\n').map_or(0, |p| p + 1);
        let line_end = src[range.end..]
            .find('\n')
            .map_or(src.len(), |p| range.end + p + 1);
        let alone = src[line_start..range.start].trim().is_empty()
            && src[range.end..line_end].trim().is_empty();
        if alone {
            // Also take one blank line that separated a removed block from the next.
            let mut end = line_end;
            if src[end..].starts_with('\n') && src[..line_start].ends_with("\n\n") {
                end += 1;
            }
            self.list.push((line_start..end, String::new()));
        } else {
            self.list.push((range, String::new()));
        }
    }
    /// Remove whole lines from the one holding `range.start` to the one holding `range.end`,
    /// for top-level blocks whose span may start after their keyword.
    fn remove_lines(&mut self, range: Range<usize>) {
        let src = self.src;
        let start = src[..range.start].rfind('\n').map_or(0, |p| p + 1);
        let mut end = line_end(src, range.end.max(range.start));
        if src[end..].starts_with('\n') && (start == 0 || src[..start].ends_with("\n\n")) {
            end += 1;
        }
        self.list.push((start..end, String::new()));
    }
    fn replace(&mut self, range: Range<usize>, text: String) {
        self.list.push((range, text));
    }
    fn insert(&mut self, at: usize, text: String) {
        self.list.push((at..at, text));
    }
    fn finish(mut self) -> Result<String> {
        self.list
            .sort_by(|a, b| a.0.start.cmp(&b.0.start).then(a.0.end.cmp(&b.0.end)));
        let mut merged: Vec<(Range<usize>, String)> = vec![];
        for (range, text) in self.list {
            if let Some((last, last_text)) = merged.last_mut() {
                if range.start < last.end {
                    if text.is_empty() && last_text.is_empty() {
                        last.end = last.end.max(range.end);
                        continue;
                    }
                    return Err(Error::InvalidEdit("overlapping changes".into()));
                }
            }
            merged.push((range, text));
        }
        let mut out = self.src.to_owned();
        for (range, text) in merged.into_iter().rev() {
            out.replace_range(range, &text);
        }
        Ok(out)
    }
}
fn line_end(src: &str, offset: usize) -> usize {
    src[offset..]
        .find('\n')
        .map_or(src.len(), |p| offset + p + 1)
}

/// Update a DBML file so its structure matches `live`, keeping everything people wrote: notes,
/// comments, aliases, groups, the project block and annotations. Tables and columns that remain
/// keep their text unless they changed; new ones are added after their siblings.
pub fn merge(src: &str, live: &Schema) -> Result<String> {
    let (file, spans) = read(src)?;
    let live_structure = structure(live);
    let mut p = Patches { src, list: vec![] };
    let live_tables: BTreeMap<String, &Table> = live_structure
        .tables
        .iter()
        .map(|t| (t.qualified(), t))
        .collect();
    let live_full: HashMap<String, &Table> =
        live.tables.iter().map(|t| (t.qualified(), t)).collect();
    let wanted: Vec<_> = live_structure.refs.iter().map(Ref::shape).collect();
    let mut placed: Vec<_> = vec![];

    // Tables that disappeared take their annotations and group entries with them.
    for t in &file.tables {
        let name = t.qualified();
        if live_tables.contains_key(&name) {
            continue;
        }
        p.remove_lines(spans.tables[&name].block.clone());
        for (range, target) in &spans.annotations {
            if *target == Target::Node(name.clone()) {
                p.list.push((range.clone(), String::new()));
            }
        }
    }
    let mut group_sizes: Vec<usize> = file.groups.iter().map(|g| g.tables.len()).collect();
    for (range, table, group) in &spans.group_items {
        if !live_tables.contains_key(table) {
            p.remove(range.clone());
            group_sizes[*group] -= 1;
        }
    }
    for (group, size) in group_sizes.iter().enumerate() {
        if *size == 0 && !file.groups[group].tables.is_empty() {
            p.remove_lines(spans.groups[group].clone());
        }
    }

    // Tables that remain: columns, inline refs and indexes.
    for t in &file.tables {
        let name = t.qualified();
        let Some(target) = live_tables.get(&name) else {
            continue;
        };
        let ts = &spans.tables[&name];
        let full = live_full[&name];
        for c in &t.columns {
            let range = ts.columns[&c.name].clone();
            let Some(want) = target.columns.iter().find(|x| x.name == c.name) else {
                p.remove(range);
                continue;
            };
            let inline_kept: Vec<Ref> = ts.inline[&c.name]
                .iter()
                .filter(|r| wanted.contains(&r.shape()))
                .cloned()
                .collect();
            placed.extend(inline_kept.iter().map(Ref::shape));
            let mut current = c.clone();
            current.note = None;
            if &current != want || inline_kept.len() != ts.inline[&c.name].len() {
                let mut column = (*want).clone();
                column.note = full
                    .columns
                    .iter()
                    .find(|x| x.name == c.name)
                    .and_then(|x| x.note.clone())
                    .or_else(|| c.note.clone());
                let inline: Vec<(String, String)> = inline_kept
                    .iter()
                    .map(|r| {
                        // Inline refs are written from this column's side.
                        if r.from.qualified_table() == name && r.from.columns == [c.name.clone()] {
                            (symbol(r.cardinality).to_owned(), endpoint_text(&r.to))
                        } else {
                            let sym = if r.cardinality == Cardinality::ManyToOne {
                                "<"
                            } else {
                                symbol(r.cardinality)
                            };
                            (sym.to_owned(), endpoint_text(&r.from))
                        }
                    })
                    .collect();
                p.replace(range, column_line(&column, &inline));
            }
        }
        let mut new_columns = String::new();
        for c in &target.columns {
            if !t.columns.iter().any(|x| x.name == c.name) {
                let mut column = c.clone();
                column.note = full
                    .columns
                    .iter()
                    .find(|x| x.name == c.name)
                    .and_then(|x| x.note.clone());
                new_columns.push_str(&format!(
                    "\n{}{}",
                    ts.column_indent,
                    column_line(&column, &[])
                ));
            }
        }
        if !new_columns.is_empty() {
            p.insert(ts.columns_end, new_columns);
        }
        let file_indexes: Vec<Index> = t
            .indexes
            .iter()
            .map(|i| Index {
                name: None,
                ..i.clone()
            })
            .collect();
        if file_indexes != target.indexes {
            let named: Vec<Index> = target
                .indexes
                .iter()
                .map(|i| {
                    let name = t
                        .indexes
                        .iter()
                        .find(|x| x.columns == i.columns)
                        .and_then(|x| x.name.clone());
                    Index { name, ..i.clone() }
                })
                .collect();
            match (&ts.indexes, named.is_empty()) {
                (Some(range), true) => p.remove(range.clone()),
                (Some(range), false) => p.replace(
                    range.clone(),
                    indexes_block(&named, &ts.column_indent)
                        .trim_start()
                        .to_owned(),
                ),
                (None, false) => p.insert(
                    ts.columns_end,
                    format!("\n\n{}", indexes_block(&named, &ts.column_indent)),
                ),
                (None, true) => {}
            }
        }
    }

    // Top-level references: keep the ones that still exist, drop the others.
    for (range, r) in &spans.ref_blocks {
        if wanted.contains(&r.shape()) && !placed.contains(&r.shape()) {
            placed.push(r.shape());
        } else {
            p.remove_lines(range.clone());
        }
    }

    // Enums.
    let key = |e: &Enum| format!("{}.{}", e.schema, e.name);
    for e in &file.enums {
        match live_structure.enums.iter().find(|x| key(x) == key(e)) {
            None => p.remove_lines(trimmed(src, &spans.enums[&key(e)])),
            Some(x) if x.values != e.values => {
                p.replace(trimmed(src, &spans.enums[&key(e)]), enum_block(x))
            }
            _ => {}
        }
    }

    // New tables, enums and references go after the last block, before trailing annotations.
    let anchor = spans
        .blocks
        .iter()
        .map(|b| trimmed(src, b).end)
        .max()
        .map_or(0, |end| line_end(src, end));
    let mut tail = String::new();
    for e in &live_structure.enums {
        if !file.enums.iter().any(|x| key(x) == key(e)) {
            tail.push_str(&format!("\n{}\n", enum_block(e)));
        }
    }
    for t in &live.tables {
        if !file.tables.iter().any(|x| x.qualified() == t.qualified()) {
            let mut table = t.clone();
            table.annotation = Annotation::default();
            tail.push_str(&format!("\n{}\n", table_block(&table)));
        }
    }
    let mut new_refs = String::new();
    for r in &live.refs {
        if !placed.contains(&r.shape()) {
            placed.push(r.shape());
            new_refs.push_str(&format!("{}\n", ref_line(r)));
        }
    }
    if !new_refs.is_empty() {
        tail.push('\n');
        tail.push_str(&new_refs);
    }
    if !tail.is_empty() {
        let lead = if anchor > 0 && !src[..anchor].ends_with('\n') {
            "\n"
        } else {
            ""
        };
        p.insert(anchor, format!("{lead}{tail}"));
    }
    let out = p.finish()?;
    // The safety net: the result must read back with exactly the live structure.
    let check = parse(&out)
        .map_err(|e| Error::InvalidEdit(format!("merging would produce invalid DBML ({e})")))?;
    if !diff(&check, live).is_empty() {
        return Err(Error::InvalidEdit(
            "the database could not be merged into this file safely".into(),
        ));
    }
    Ok(out)
}

/// Set or clear the annotation of a table (`Target::Node("schema.table")`) or of the schema.
pub fn annotate(src: &str, target: &Target, annotation: &Annotation) -> Result<String> {
    let (schema, spans) = read(src)?;
    match target {
        Target::Diagram => {}
        Target::Node(name) if schema.tables.iter().any(|t| &t.qualified() == name) => {}
        Target::Node(name) => return Err(Error::InvalidEdit(format!("no table `{name}`"))),
        Target::Edge(_) => {
            return Err(Error::InvalidEdit(
                "DBML annotations attach to tables or the schema".into(),
            ))
        }
    }
    for code in &annotation.code {
        code.validate()?;
    }
    let line = annotate::format_comment(COMMENT, target, annotation);
    let mut out = src.to_owned();
    match spans.annotations.iter().find(|(_, t)| t == target) {
        Some((range, _)) if annotation.is_empty() => out.replace_range(range.clone(), ""),
        Some((range, _)) => {
            let newline = if src[range.clone()].ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.replace_range(range.clone(), &format!("{line}{newline}"));
        }
        None if annotation.is_empty() => {}
        None => {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            if spans.annotations.is_empty() {
                out.push('\n');
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_defaults_are_written_as_expressions() {
        let column = Column {
            name: "x".into(),
            data_type: "int".into(),
            default: Some("-1".into()),
            ..Column::default()
        };
        let line = column_line(&column, &[]);
        assert_eq!(line, "x int [default: `-1`]");
        let schema = parse(&format!("Table t {{\n  {line}\n}}")).unwrap();
        assert_eq!(schema.tables[0].columns[0].default.as_deref(), Some("`-1`"));
    }
}
