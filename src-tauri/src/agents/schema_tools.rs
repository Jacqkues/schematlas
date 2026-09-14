//! Compact plain-text views of a project's catalog for coding agents.
//!
//! Every function here is pure: it reads an already imported [`Source`] and
//! returns plain text (never JSON) sized for an agent context window — one line
//! per entity, deterministic ordering, descriptions capped at 120 characters.
//! Nothing in this module opens a connection, so no credential or connection
//! string can reach the rendered output.
use crate::domain::{Entity, Field, Project, Relation, Source};
use crate::error::{AppError, Result};
use std::collections::{HashMap, HashSet, VecDeque};

/// Longest description rendered; longer text is truncated with an ellipsis.
const MAX_TEXT: usize = 120;
/// Canvas card width shared with the UI layout.
const CARD_WIDTH: u32 = 284;
/// Longest foreign-key path [`join_path`] will look for.
const MAX_HOPS: usize = 6;
/// Suggestions listed when a reference cannot be resolved.
const MAX_CANDIDATES: usize = 5;
const OPERATION: &str = "operation";
const SCHEMA: &str = "schema";
const VIEW: &str = "view";
const OPENAPI: &str = "openapi";

/// Human-friendly reference: `namespace.name` for tables/views/models, `METHOD route` for operations.
pub fn display_ref(entity: &Entity) -> String {
    match (entity.kind.as_str(), entity.method.as_deref()) {
        (OPERATION, Some(method)) => format!("{method} {}", entity.name),
        _ => format!("{}.{}", entity.namespace, entity.name),
    }
}

/// Resolve a user/agent reference to an entity. Accept, in order: the exact internal id (JSON tuple string),
/// `namespace.name` (split on the LAST dot first, then try the FIRST dot), a bare `name` when it is unique in the
/// source, and for operations `METHOD route` (case-insensitive method) or the bare route when unique.
/// On failure return AppError::Validation with a short message listing up to 5 close candidates (same name or
/// containing the text), e.g. "Unknown table `orders`. Did you mean: main.orders, sales.orders?".
pub fn resolve_entity<'a>(source: &'a Source, reference: &str) -> Result<&'a Entity> {
    let text = reference.trim();
    let entities = &source.graph.entities;
    if text.is_empty() {
        return Err(AppError::Validation(format!(
            "Name the {} to inspect.",
            noun(source)
        )));
    }
    if let Some(entity) = entities.iter().find(|e| e.id == text) {
        return Ok(entity);
    }
    for (namespace, name) in [text.rsplit_once('.'), text.split_once('.')]
        .into_iter()
        .flatten()
    {
        if let Some(entity) = entities
            .iter()
            .find(|e| e.namespace == namespace && e.name == name)
        {
            return Ok(entity);
        }
    }
    if let Some(entity) = unique(entities.iter().filter(|e| e.name == text)) {
        return Ok(entity);
    }
    if let Some((method, route)) = text.split_once(char::is_whitespace) {
        let method = method.trim().to_ascii_uppercase();
        let route = route.trim();
        if let Some(entity) = unique(entities.iter().filter(|e| {
            e.kind == OPERATION
                && e.name == route
                && e.method.as_deref().map(str::to_ascii_uppercase).as_deref() == Some(&method)
        })) {
            return Ok(entity);
        }
    }
    Err(AppError::Validation(unknown(
        noun(source),
        text,
        candidates(entities.iter(), text),
    )))
}

/// Resolve a source by exact id, or by exact/unique case-insensitive name. Same error style.
pub fn resolve_source<'a>(project: &'a Project, reference: &str) -> Result<&'a Source> {
    let text = reference.trim();
    if text.is_empty() {
        return Err(AppError::Validation("Name the source to inspect.".into()));
    }
    let sources = &project.sources;
    if let Some(source) = sources.iter().find(|s| s.id == text) {
        return Ok(source);
    }
    if let Some(source) = unique(sources.iter().filter(|s| s.name == text)) {
        return Ok(source);
    }
    if let Some(source) = unique(sources.iter().filter(|s| s.name.eq_ignore_ascii_case(text))) {
        return Ok(source);
    }
    let lowered = text.to_lowercase();
    let mut names: Vec<&str> = sources
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&lowered))
        .map(|s| s.name.as_str())
        .collect();
    names.sort_unstable();
    names.truncate(MAX_CANDIDATES);
    Err(AppError::Validation(unknown(
        "source",
        text,
        names.into_iter().map(str::to_owned).collect(),
    )))
}

/// Compact listing of a source, one line per entity, paginated.
pub fn render_schema(
    source: &Source,
    namespace: Option<&str>,
    offset: usize,
    limit: usize,
) -> String {
    let api = is_api(source);
    let entities = ordered(
        source
            .graph
            .entities
            .iter()
            .filter(|e| namespace.is_none_or(|ns| e.namespace == ns)),
    );
    let total = entities.len();
    let page: Vec<&&Entity> = entities
        .iter()
        .skip(offset)
        .take(page_size(limit))
        .collect();
    let mut out = format!("{} of {total} entities in {}", page.len(), source.name);
    if let Some(namespace) = namespace {
        out.push_str(&format!(" namespace={namespace}"));
    }
    out.push_str(&format!(" (offset {offset})."));
    let next = offset.saturating_add(page.len());
    if next < total {
        out.push_str(&format!(" Next: offset={next}"));
    }
    if page.is_empty() {
        return out;
    }
    let by_id = index(source);
    let outgoing = group(source.graph.relations.iter(), |r| r.source.as_str());
    for entity in page {
        out.push('\n');
        let relations = outgoing.get(entity.id.as_str());
        if entity.kind == VIEW {
            out.push_str("view ");
        }
        out.push_str(&listing_ref(entity));
        if entity.kind != OPERATION {
            let foreign = foreign_keys(relations, &by_id);
            let (single, composite) = unique_keys(entity);
            out.push_str(" (");
            for (index, field) in entity.fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                out.push_str(&column(
                    field,
                    foreign.get(field.name.as_str()).map(String::as_str),
                    single.contains(&field.name.as_str()),
                    api,
                ));
            }
            out.push(')');
            for key in composite {
                out.push_str(&format!(" UNIQUE({})", key.join(", ")));
            }
        }
        let targets: Vec<String> = relations
            .into_iter()
            .flatten()
            .filter(|r| r.source_field.is_none())
            .filter_map(|r| by_id.get(r.target.as_str()))
            .map(|e| listing_ref(e))
            .collect();
        if !targets.is_empty() {
            out.push_str(&format!(" -> {}", targets.join(", ")));
        }
        if let Some(description) = described(entity.description.as_deref()) {
            out.push_str(&format!(" -- {description}"));
        }
    }
    out
}

/// Case-insensitive search across ALL sources of the project on namespace, table/model/operation names and column names.
/// An empty query is a validation error; otherwise at most `limit` lines (1..=100, 50 when 0) grouped by source.
pub fn search(project: &Project, query: &str, limit: usize) -> Result<String> {
    let query = query.trim();
    if query.is_empty() {
        return Err(AppError::Validation("Enter a search term.".into()));
    }
    let limit = if limit == 0 { 50 } else { limit.clamp(1, 100) };
    let needle = query.to_lowercase();
    let mut lines: Vec<String> = vec![];
    let mut total = 0usize;
    for source in &project.sources {
        for entity in ordered(source.graph.entities.iter()) {
            let reference = display_ref(entity);
            let mut hits: Vec<String> = vec![];
            if contains(&entity.namespace, &needle) || contains(&entity.name, &needle) {
                hits.push(format!("{} › {reference}", source.name));
            }
            for field in entity.fields.iter().filter(|f| contains(&f.name, &needle)) {
                hits.push(format!(
                    "{} › {reference}.{} {}",
                    source.name,
                    field.name,
                    data_type(field)
                ));
            }
            total += hits.len();
            lines.extend(hits.into_iter().take(limit.saturating_sub(lines.len())));
        }
    }
    let mut out = format!("{total} matches for \"{query}\"");
    if total > lines.len() {
        out.push_str(&format!(" (showing first {})", lines.len()));
    }
    for line in lines {
        out.push('\n');
        out.push_str(&line);
    }
    Ok(out)
}

/// Everything about one entity, compact.
pub fn describe(source: &Source, entity: &Entity) -> String {
    let api = is_api(source);
    let by_id = index(source);
    let outgoing: Vec<&Relation> = source
        .graph
        .relations
        .iter()
        .filter(|r| r.source == entity.id)
        .collect();
    let incoming: Vec<&Relation> = source
        .graph
        .relations
        .iter()
        .filter(|r| r.target == entity.id)
        .collect();
    let mut out = if entity.kind == OPERATION {
        format!("{} ({OPERATION})", display_ref(entity))
    } else {
        format!(
            "{} ({}, {} columns)",
            display_ref(entity),
            entity.kind,
            entity.fields.len()
        )
    };
    if let Some(description) = described(entity.description.as_deref()) {
        out.push('\n');
        out.push_str(&description);
    }
    if !entity.fields.is_empty() {
        let foreign = foreign_keys(Some(&outgoing), &by_id);
        let (single, _) = unique_keys(entity);
        out.push_str("\ncolumns:");
        for field in &entity.fields {
            out.push_str("\n  ");
            out.push_str(&column(
                field,
                foreign.get(field.name.as_str()).map(String::as_str),
                single.contains(&field.name.as_str()),
                api,
            ));
            if let Some(description) = described(field.description.as_deref()) {
                out.push_str(&format!("  -- {description}"));
            }
        }
    }
    let (_, composite) = unique_keys(entity);
    if !composite.is_empty() {
        out.push_str("\nunique keys:");
        for key in composite {
            out.push_str(&format!("\n  {}", key.join(", ")));
        }
    }
    if !outgoing.is_empty() {
        out.push_str("\nreferences (outgoing):");
        for relation in &outgoing {
            let target = by_id
                .get(relation.target.as_str())
                .map(|e| listing_ref(e))
                .unwrap_or_else(|| relation.target.clone());
            out.push_str("\n  ");
            match (&relation.source_field, &relation.target_field) {
                (Some(from), Some(to)) => out.push_str(&format!("{from} -> {target}.{to}")),
                (Some(from), None) => out.push_str(&format!("{from} -> {target}")),
                _ => out.push_str(&format!("-> {target} ({})", one_line(&relation.label))),
            }
        }
    }
    if !incoming.is_empty() {
        out.push_str("\nreferenced by (incoming):");
        for relation in &incoming {
            let origin = by_id
                .get(relation.source.as_str())
                .map(|e| listing_ref(e))
                .unwrap_or_else(|| relation.source.clone());
            out.push_str("\n  ");
            match (&relation.source_field, &relation.target_field) {
                (Some(from), Some(to)) => out.push_str(&format!("{origin}.{from} -> {to}")),
                (Some(from), None) => out.push_str(&format!("{origin}.{from} ->")),
                _ => out.push_str(&format!("{origin} -> ({})", one_line(&relation.label))),
            }
        }
    }
    out
}

/// Shortest foreign-key path between two entities (BFS over relations in both directions, max 6 hops).
pub fn join_path(source: &Source, from: &Entity, to: &Entity) -> String {
    if from.id == to.id {
        return "Same table.".into();
    }
    let by_id = index(source);
    let Some(path) = shortest_path(source, &from.id, &to.id) else {
        return format!(
            "No foreign-key path between {} and {} within {MAX_HOPS} joins.",
            display_ref(from),
            display_ref(to)
        );
    };
    let mut out = format!(
        "{} -> {} in {} joins:",
        display_ref(from),
        display_ref(to),
        path.len()
    );
    let reference = |id: &str| {
        by_id
            .get(id)
            .map(|e| display_ref(e))
            .unwrap_or_else(|| id.to_owned())
    };
    let mut sql = format!("\n\nSELECT * FROM {}", display_ref(from));
    let mut node = from.id.as_str();
    for hop in &path {
        let origin = reference(&hop.relation.source);
        let target = reference(&hop.relation.target);
        let next = if hop.forward {
            hop.relation.target.as_str()
        } else {
            hop.relation.source.as_str()
        };
        debug_assert_ne!(node, next);
        node = next;
        match (&hop.relation.source_field, &hop.relation.target_field) {
            (Some(left), Some(right)) => {
                out.push_str(&format!("\n  {origin}.{left} -> {target}.{right}"));
                sql.push_str(&format!(
                    " JOIN {} ON {origin}.{left} = {target}.{right}",
                    reference(next)
                ));
            }
            _ => {
                out.push_str(&format!(
                    "\n  {origin} -> {target} ({})",
                    one_line(&hop.relation.label)
                ));
                sql.push_str(&format!(
                    " JOIN {} ON /* composite key: {} */",
                    reference(next),
                    one_line(&hop.relation.label)
                ));
            }
        }
    }
    out.push_str(&sql);
    out
}

/// Compact canvas state: positioned nodes, unpositioned nodes and group overlays.
pub fn render_canvas(source: &Source) -> String {
    let entities = ordered(source.graph.entities.iter());
    let by_id = index(source);
    let positioned: Vec<(&&Entity, &crate::domain::Position)> = entities
        .iter()
        .filter_map(|e| source.positions.get(&e.id).map(|p| (e, p)))
        .collect();
    let mut out = format!(
        "{} of {} nodes positioned; {} groups; card width {CARD_WIDTH}",
        positioned.len(),
        entities.len(),
        source.groups.len()
    );
    if !positioned.is_empty() {
        out.push_str("\nnodes:");
        for (entity, position) in &positioned {
            out.push_str(&format!(
                "\n  {} {} {}",
                display_ref(entity),
                position.x.round() as i64,
                position.y.round() as i64
            ));
        }
    }
    let missing: Vec<String> = entities
        .iter()
        .filter(|e| !source.positions.contains_key(&e.id))
        .map(|e| display_ref(e))
        .collect();
    if !missing.is_empty() {
        out.push_str(&format!("\nunpositioned: {}", missing.join(", ")));
    }
    if !source.groups.is_empty() {
        out.push_str("\ngroups:");
        for group in &source.groups {
            let members: Vec<String> = group
                .node_ids
                .iter()
                .map(|id| {
                    by_id
                        .get(id.as_str())
                        .map(|e| display_ref(e))
                        .unwrap_or_else(|| id.clone())
                })
                .collect();
            out.push_str(&format!(
                "\n  {} \"{}\" {}: {}",
                group.id,
                one_line(&group.name),
                group.color,
                members.join(", ")
            ));
        }
    }
    out
}

/// One BFS step: the relation traversed and whether it was followed source -> target.
struct Hop<'a> {
    relation: &'a Relation,
    forward: bool,
}

fn shortest_path<'a>(source: &'a Source, from: &str, to: &str) -> Option<Vec<Hop<'a>>> {
    let mut adjacency: HashMap<&str, Vec<(&str, Hop)>> = HashMap::new();
    for relation in &source.graph.relations {
        adjacency.entry(&relation.source).or_default().push((
            &relation.target,
            Hop {
                relation,
                forward: true,
            },
        ));
        if relation.target != relation.source {
            adjacency.entry(&relation.target).or_default().push((
                &relation.source,
                Hop {
                    relation,
                    forward: false,
                },
            ));
        }
    }
    let mut previous: HashMap<&str, (&str, &Hop)> = HashMap::new();
    let mut seen: HashSet<&str> = HashSet::from([from]);
    let mut queue = VecDeque::from([(from, 0usize)]);
    let mut found = false;
    while let Some((node, depth)) = queue.pop_front() {
        if depth == MAX_HOPS {
            continue;
        }
        for (next, hop) in adjacency.get(node).into_iter().flatten() {
            if !seen.insert(next) {
                continue;
            }
            previous.insert(next, (node, hop));
            if *next == to {
                found = true;
                break;
            }
            queue.push_back((next, depth + 1));
        }
        if found {
            break;
        }
    }
    if !found {
        return None;
    }
    let mut path = vec![];
    let mut node = to;
    while let Some((origin, hop)) = previous.get(node) {
        path.push(Hop {
            relation: hop.relation,
            forward: hop.forward,
        });
        node = origin;
    }
    path.reverse();
    Some(path)
}

fn noun(source: &Source) -> &'static str {
    if is_api(source) {
        "entity"
    } else {
        "table"
    }
}

fn is_api(source: &Source) -> bool {
    source.kind == OPENAPI
}

/// OpenAPI models read best unqualified; everything else keeps its namespace.
fn listing_ref(entity: &Entity) -> String {
    if entity.kind == SCHEMA {
        entity.name.clone()
    } else {
        display_ref(entity)
    }
}

fn unknown(noun: &str, text: &str, candidates: Vec<String>) -> String {
    let mut message = format!("Unknown {noun} `{text}`.");
    if !candidates.is_empty() {
        message.push_str(&format!(" Did you mean: {}?", candidates.join(", ")));
    }
    message
}

/// Suggestions, most useful tier first: the same name in another namespace, then a
/// containing reference, then a near miss such as `itemz` for `items`.
fn candidates<'a>(entities: impl Iterator<Item = &'a Entity>, text: &str) -> Vec<String> {
    let needle = text.to_lowercase();
    let entities = ordered(entities);
    for tier in [0u8, 1, 2] {
        let mut close: Vec<&Entity> = entities
            .iter()
            .copied()
            .filter(|e| match tier {
                0 => e.name.eq_ignore_ascii_case(text),
                1 => {
                    contains(&e.name, &needle)
                        || needle.contains(&e.name.to_lowercase())
                        || contains(&e.namespace, &needle)
                        || display_ref(e).to_lowercase().contains(&needle)
                }
                _ => near(&e.name.to_lowercase(), &needle),
            })
            .collect();
        if !close.is_empty() {
            close.truncate(MAX_CANDIDATES);
            return close.into_iter().map(display_ref).collect();
        }
    }
    vec![]
}

/// Cheap typo tolerance: one edit for short names, two once both are longer.
fn near(left: &str, right: &str) -> bool {
    let (left, right): (Vec<char>, Vec<char>) = (left.chars().collect(), right.chars().collect());
    let budget = if left.len().min(right.len()) > 4 {
        2
    } else {
        1
    };
    if left.len().abs_diff(right.len()) > budget {
        return false;
    }
    let mut row: Vec<usize> = (0..=right.len()).collect();
    for (i, a) in left.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, b) in right.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = (previous + usize::from(a != b))
                .min(current + 1)
                .min(row[j] + 1);
            previous = current;
        }
    }
    row[right.len()] <= budget
}

fn unique<T>(mut matches: impl Iterator<Item = T>) -> Option<T> {
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn contains(haystack: &str, lowercase_needle: &str) -> bool {
    haystack.to_lowercase().contains(lowercase_needle)
}

fn page_size(limit: usize) -> usize {
    if limit == 0 {
        100
    } else {
        limit.clamp(1, 200)
    }
}

fn ordered<'a>(entities: impl Iterator<Item = &'a Entity>) -> Vec<&'a Entity> {
    let mut entities: Vec<&Entity> = entities.collect();
    entities.sort_by(|a, b| (&a.namespace, &a.name, &a.id).cmp(&(&b.namespace, &b.name, &b.id)));
    entities
}

fn index(source: &Source) -> HashMap<&str, &Entity> {
    source
        .graph
        .entities
        .iter()
        .map(|e| (e.id.as_str(), e))
        .collect()
}

fn group<'a, T>(
    items: impl Iterator<Item = &'a T>,
    key: impl Fn(&'a T) -> &'a str,
) -> HashMap<&'a str, Vec<&'a T>> {
    let mut grouped: HashMap<&str, Vec<&T>> = HashMap::new();
    for item in items {
        grouped.entry(key(item)).or_default().push(item);
    }
    grouped
}

/// `column name -> ns.table.column` for the relations that name a source column.
fn foreign_keys(
    relations: Option<&Vec<&Relation>>,
    by_id: &HashMap<&str, &Entity>,
) -> HashMap<String, String> {
    let mut foreign = HashMap::new();
    for relation in relations.into_iter().flatten() {
        let Some(column) = &relation.source_field else {
            continue;
        };
        let Some(target) = by_id.get(relation.target.as_str()) else {
            continue;
        };
        let mut reference = listing_ref(target);
        if let Some(field) = &relation.target_field {
            reference.push('.');
            reference.push_str(field);
        }
        foreign.entry(column.clone()).or_insert(reference);
    }
    foreign
}

/// Single-column unique constraints and composite ones, ignoring the primary key itself.
fn unique_keys(entity: &Entity) -> (Vec<&str>, Vec<&Vec<String>>) {
    let primary: Vec<&str> = entity
        .fields
        .iter()
        .filter(|f| f.primary_key)
        .map(|f| f.name.as_str())
        .collect();
    let mut single: Vec<&str> = vec![];
    let mut composite: Vec<&Vec<String>> = vec![];
    for key in entity.unique_keys.iter().flatten() {
        if key.is_empty()
            || (key.len() == primary.len() && key.iter().all(|c| primary.contains(&c.as_str())))
        {
            continue;
        }
        match key.as_slice() {
            [column] if !primary.contains(&column.as_str()) => {
                if !single.contains(&column.as_str()) {
                    single.push(column);
                }
            }
            [_] => {}
            _ => composite.push(key),
        }
    }
    (single, composite)
}

fn data_type(field: &Field) -> &str {
    if field.data_type.is_empty() {
        "any"
    } else {
        &field.data_type
    }
}

/// `name TYPE PK FK->ns.table.col UNIQUE NOT NULL DEFAULT x NULL`, skipping what does not apply.
fn column(field: &Field, foreign: Option<&str>, unique: bool, api: bool) -> String {
    let mut out = format!("{} {}", field.name, data_type(field));
    if field.primary_key {
        out.push_str(" PK");
    }
    if let Some(target) = foreign {
        out.push_str(&format!(" FK->{target}"));
    }
    if unique {
        out.push_str(" UNIQUE");
    }
    // A primary key is implicitly NOT NULL; API fields only carry `required`.
    if (api && field.required) || (!api && !field.nullable && !field.primary_key) {
        out.push_str(" NOT NULL");
    }
    if let Some(default) = &field.default_value {
        out.push_str(&format!(" DEFAULT {}", one_line(default)));
    }
    if !api && field.nullable && !field.primary_key {
        out.push_str(" NULL");
    }
    out
}

fn described(description: Option<&str>) -> Option<String> {
    description.map(short).filter(|text| !text.is_empty())
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// One line, at most [`MAX_TEXT`] characters, ellipsis when cut.
fn short(text: &str) -> String {
    let text = one_line(text);
    if text.chars().count() <= MAX_TEXT {
        return text;
    }
    let mut out: String = text.chars().take(MAX_TEXT - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{entity_id, CanvasGroup, Graph, Position};
    use std::collections::BTreeMap;

    fn field(name: &str, data_type: &str, nullable: bool, primary_key: bool) -> Field {
        Field {
            name: name.into(),
            data_type: data_type.into(),
            nullable,
            primary_key,
            required: !nullable,
            default_value: None,
            description: None,
        }
    }
    fn defaulted(mut field: Field, value: &str) -> Field {
        field.default_value = Some(value.into());
        field
    }
    fn documented(mut field: Field, text: &str) -> Field {
        field.description = Some(text.into());
        field
    }
    fn table(namespace: &str, name: &str, kind: &str, fields: Vec<Field>) -> Entity {
        Entity {
            id: entity_id(namespace, name),
            name: name.into(),
            namespace: namespace.into(),
            kind: kind.into(),
            description: None,
            fields,
            method: None,
            unique_keys: None,
        }
    }
    fn relation(
        from: &str,
        from_field: Option<&str>,
        to: &str,
        to_field: Option<&str>,
    ) -> Relation {
        Relation {
            id: format!("{from}-{to}"),
            source: from.into(),
            target: to.into(),
            source_field: from_field.map(str::to_owned),
            target_field: to_field.map(str::to_owned),
            label: "fk".into(),
        }
    }
    fn source(name: &str, kind: &str, graph: Graph) -> Source {
        Source {
            groups: vec![],
            layout_backup: None,
            api_base_url: None,
            id: format!("source-{name}"),
            name: name.into(),
            kind: kind.into(),
            database_kind: None,
            imported_at: "2024-01-01T00:00:00Z".into(),
            graph,
            positions: BTreeMap::new(),
        }
    }

    /// Three tables (foreign keys, a default, single and composite unique keys) plus a documented view.
    fn shop() -> Source {
        let mut customers = table(
            "main",
            "customers",
            "table",
            vec![
                field("id", "INTEGER", false, true),
                field("email", "TEXT", false, false),
                field("name", "TEXT", true, false),
            ],
        );
        customers.unique_keys = Some(vec![vec!["id".into()], vec!["email".into()]]);
        let orders = table(
            "main",
            "orders",
            "table",
            vec![
                field("id", "INTEGER", false, true),
                documented(
                    field("customer_id", "INTEGER", false, false),
                    "Owning customer.",
                ),
                defaulted(field("status", "TEXT", false, false), "'pending'"),
            ],
        );
        let mut items = table(
            "main",
            "order_items",
            "table",
            vec![
                field("order_id", "INTEGER", false, false),
                field("sku", "TEXT", false, false),
                defaulted(field("qty", "INTEGER", false, false), "1"),
            ],
        );
        items.unique_keys = Some(vec![vec!["order_id".into(), "sku".into()]]);
        let mut totals = table(
            "sales",
            "order_totals",
            "view",
            vec![
                field("order_id", "INTEGER", true, false),
                field("total", "REAL", true, false),
            ],
        );
        totals.description = Some("Revenue per order.".into());
        let graph = Graph {
            relations: vec![
                relation(&orders.id, Some("customer_id"), &customers.id, Some("id")),
                relation(&items.id, Some("order_id"), &orders.id, Some("id")),
                relation(&totals.id, None, &orders.id, None),
            ],
            entities: vec![customers, orders, items, totals],
            warnings: vec![],
        };
        source("shop", "database", graph)
    }

    /// One documented operation referencing two models.
    fn api() -> Source {
        let operation = Entity {
            id: "operation:get:/pets/{id}".into(),
            name: "/pets/{id}".into(),
            namespace: "Endpoints".into(),
            kind: "operation".into(),
            description: Some("Read one pet.\n\nBy identifier.".into()),
            fields: vec![
                field("path: id", "integer", false, false),
                Field {
                    name: "response 200".into(),
                    data_type: "Pet".into(),
                    ..Field::default()
                },
            ],
            method: Some("GET".into()),
            unique_keys: None,
        };
        let pet = Entity {
            id: "#/components/schemas/Pet".into(),
            name: "Pet".into(),
            namespace: "Schemas".into(),
            kind: "schema".into(),
            description: None,
            fields: vec![
                Field {
                    name: "id".into(),
                    data_type: "integer".into(),
                    ..Field::default()
                },
                Field {
                    name: "name".into(),
                    data_type: "string".into(),
                    required: true,
                    ..Field::default()
                },
            ],
            method: None,
            unique_keys: None,
        };
        let error = Entity {
            id: "#/components/schemas/Error".into(),
            name: "Error".into(),
            namespace: "Schemas".into(),
            kind: "schema".into(),
            description: None,
            fields: vec![Field {
                name: "message".into(),
                data_type: String::new(),
                ..Field::default()
            }],
            method: None,
            unique_keys: None,
        };
        let reference = |target: &str| Relation {
            id: format!("reference:{target}"),
            source: operation.id.clone(),
            target: target.into(),
            source_field: None,
            target_field: None,
            label: "$ref".into(),
        };
        let graph = Graph {
            relations: vec![reference(&pet.id), reference(&error.id)],
            entities: vec![operation, pet, error],
            warnings: vec![],
        };
        source("petstore", "openapi", graph)
    }

    fn find<'a>(source: &'a Source, reference: &str) -> &'a Entity {
        resolve_entity(source, reference).unwrap()
    }

    #[test]
    fn display_ref_covers_tables_and_operations() {
        let shop = shop();
        assert_eq!(display_ref(find(&shop, "main.orders")), "main.orders");
        let api = api();
        assert_eq!(
            display_ref(find(&api, "operation:get:/pets/{id}")),
            "GET /pets/{id}"
        );
        assert_eq!(display_ref(find(&api, "Pet")), "Schemas.Pet");
    }

    #[test]
    fn resolve_entity_accepts_ids_qualified_and_bare_names() {
        let shop = shop();
        assert_eq!(find(&shop, &entity_id("main", "orders")).name, "orders");
        assert_eq!(find(&shop, "main.orders").namespace, "main");
        assert_eq!(find(&shop, " sales.order_totals ").name, "order_totals");
        assert_eq!(find(&shop, "orders").id, entity_id("main", "orders"));
    }

    #[test]
    fn resolve_entity_splits_on_the_last_then_the_first_dot() {
        let mut shop = shop();
        // A namespace containing a dot only resolves through the first-dot fallback.
        shop.graph
            .entities
            .push(table("audit.v2", "events", "table", vec![]));
        assert_eq!(find(&shop, "audit.v2.events").namespace, "audit.v2");
        // A name containing a dot resolves through the last-dot split.
        shop.graph
            .entities
            .push(table("main", "v2.events", "table", vec![]));
        assert_eq!(find(&shop, "main.v2.events").name, "v2.events");
    }

    #[test]
    fn resolve_entity_reports_candidates_when_ambiguous_or_unknown() {
        let mut shop = shop();
        shop.graph.entities.push(table(
            "sales",
            "orders",
            "table",
            vec![field("id", "INTEGER", false, true)],
        ));
        let error = resolve_entity(&shop, "orders").unwrap_err().to_string();
        assert_eq!(
            error,
            "Unknown table `orders`. Did you mean: main.orders, sales.orders?"
        );
        assert_eq!(
            resolve_entity(&shop, "nothing_here")
                .unwrap_err()
                .to_string(),
            "Unknown table `nothing_here`."
        );
        let close = resolve_entity(&shop, "order_item").unwrap_err().to_string();
        assert_eq!(
            close,
            "Unknown table `order_item`. Did you mean: main.order_items?"
        );
        // A typo still reaches the intended table.
        assert_eq!(
            resolve_entity(&shop, "custumers").unwrap_err().to_string(),
            "Unknown table `custumers`. Did you mean: main.customers?"
        );
    }

    #[test]
    fn resolve_entity_accepts_method_and_route_for_operations() {
        let mut api = api();
        assert_eq!(find(&api, "GET /pets/{id}").kind, "operation");
        assert_eq!(find(&api, "get /pets/{id}").method.as_deref(), Some("GET"));
        // The bare route is enough while it is unique.
        assert_eq!(find(&api, "/pets/{id}").method.as_deref(), Some("GET"));
        let mut deletion = api.graph.entities[0].clone();
        deletion.id = "operation:delete:/pets/{id}".into();
        deletion.method = Some("DELETE".into());
        api.graph.entities.push(deletion);
        assert!(resolve_entity(&api, "/pets/{id}").is_err());
        assert_eq!(
            find(&api, "delete /pets/{id}").id,
            "operation:delete:/pets/{id}"
        );
    }

    #[test]
    fn resolve_source_matches_id_then_name() {
        let project = Project {
            working_directory: None,
            id: "p".into(),
            name: "Commerce".into(),
            description: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            sources: vec![shop(), api()],
        };
        assert_eq!(
            resolve_source(&project, "source-shop").unwrap().name,
            "shop"
        );
        assert_eq!(resolve_source(&project, "shop").unwrap().id, "source-shop");
        assert_eq!(
            resolve_source(&project, "PetStore").unwrap().name,
            "petstore"
        );
        assert_eq!(
            resolve_source(&project, "store").unwrap_err().to_string(),
            "Unknown source `store`. Did you mean: petstore?"
        );
        assert_eq!(
            resolve_source(&project, "nope").unwrap_err().to_string(),
            "Unknown source `nope`."
        );
    }

    #[test]
    fn render_schema_lines_are_compact_and_ordered() {
        let rendered = render_schema(&shop(), None, 0, 0);
        assert_eq!(
            rendered,
            "4 of 4 entities in shop (offset 0).\n\
             main.customers (id INTEGER PK, email TEXT UNIQUE NOT NULL, name TEXT NULL)\n\
             main.order_items (order_id INTEGER FK->main.orders.id NOT NULL, sku TEXT NOT NULL, qty INTEGER NOT NULL DEFAULT 1) UNIQUE(order_id, sku)\n\
             main.orders (id INTEGER PK, customer_id INTEGER FK->main.customers.id NOT NULL, status TEXT NOT NULL DEFAULT 'pending')\n\
             view sales.order_totals (order_id INTEGER NULL, total REAL NULL) -> main.orders -- Revenue per order."
        );
    }

    #[test]
    fn render_schema_paginates_and_filters_namespaces() {
        let shop = shop();
        let first = render_schema(&shop, None, 0, 2);
        assert_eq!(
            first.lines().next().unwrap(),
            "2 of 4 entities in shop (offset 0). Next: offset=2"
        );
        assert_eq!(first.lines().count(), 3);
        let last = render_schema(&shop, None, 2, 2);
        assert_eq!(
            last.lines().next().unwrap(),
            "2 of 4 entities in shop (offset 2)."
        );
        let filtered = render_schema(&shop, Some("main"), 0, 2);
        assert_eq!(
            filtered.lines().next().unwrap(),
            "2 of 3 entities in shop namespace=main (offset 0). Next: offset=2"
        );
        assert_eq!(
            render_schema(&shop, Some("nowhere"), 0, 0),
            "0 of 0 entities in shop namespace=nowhere (offset 0)."
        );
        // Clamped: 0 means 100, oversized limits stop at 200, and both fit this fixture.
        assert_eq!(render_schema(&shop, None, 0, 9_000).lines().count(), 5);
    }

    #[test]
    fn render_schema_renders_operations_and_models() {
        assert_eq!(
            render_schema(&api(), None, 0, 0),
            "3 of 3 entities in petstore (offset 0).\n\
             GET /pets/{id} -> Pet, Error -- Read one pet. By identifier.\n\
             Error (message any)\n\
             Pet (id integer, name string NOT NULL)"
        );
    }

    #[test]
    fn descriptions_are_truncated_to_120_characters() {
        let mut shop = shop();
        shop.graph.entities[0].description = Some("x".repeat(400));
        let line = render_schema(&shop, Some("main"), 0, 1);
        let description = line.rsplit(" -- ").next().unwrap();
        assert_eq!(description.chars().count(), MAX_TEXT);
        assert!(description.ends_with('…'));
    }

    #[test]
    fn search_lists_entity_and_column_matches() {
        let project = Project {
            working_directory: None,
            id: "p".into(),
            name: "Commerce".into(),
            description: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            sources: vec![shop(), api()],
        };
        assert_eq!(
            search(&project, "EMAIL", 0).unwrap(),
            "1 matches for \"EMAIL\"\nshop › main.customers.email TEXT"
        );
        assert_eq!(
            search(&project, "pet", 0).unwrap(),
            "2 matches for \"pet\"\npetstore › GET /pets/{id}\npetstore › Schemas.Pet"
        );
        assert_eq!(
            search(&project, "order", 3).unwrap(),
            "5 matches for \"order\" (showing first 3)\n\
             shop › main.order_items\n\
             shop › main.order_items.order_id INTEGER\n\
             shop › main.orders"
        );
        assert_eq!(search(&project, "zzz", 0).unwrap(), "0 matches for \"zzz\"");
        assert_eq!(
            search(&project, "  ", 0).unwrap_err().to_string(),
            "Enter a search term."
        );
    }

    #[test]
    fn describe_reports_columns_relations_and_unique_keys() {
        let shop = shop();
        assert_eq!(
            describe(&shop, find(&shop, "main.orders")),
            "main.orders (table, 3 columns)\n\
             columns:\n\
             \x20 id INTEGER PK\n\
             \x20 customer_id INTEGER FK->main.customers.id NOT NULL  -- Owning customer.\n\
             \x20 status TEXT NOT NULL DEFAULT 'pending'\n\
             references (outgoing):\n\
             \x20 customer_id -> main.customers.id\n\
             referenced by (incoming):\n\
             \x20 main.order_items.order_id -> id\n\
             \x20 sales.order_totals -> (fk)"
        );
        assert_eq!(
            describe(&shop, find(&shop, "order_items")),
            "main.order_items (table, 3 columns)\n\
             columns:\n\
             \x20 order_id INTEGER FK->main.orders.id NOT NULL\n\
             \x20 sku TEXT NOT NULL\n\
             \x20 qty INTEGER NOT NULL DEFAULT 1\n\
             unique keys:\n\
             \x20 order_id, sku\n\
             references (outgoing):\n\
             \x20 order_id -> main.orders.id"
        );
        // A view with an unqualified relation keeps the incoming/outgoing sections readable.
        assert!(describe(&shop, find(&shop, "order_totals")).contains("\n  -> main.orders (fk)"));
        assert!(!describe(&shop, find(&shop, "main.customers")).contains("unique keys:"));
    }

    #[test]
    fn describe_lists_operation_references() {
        let api = api();
        assert_eq!(
            describe(&api, find(&api, "GET /pets/{id}")),
            "GET /pets/{id} (operation)\n\
             Read one pet. By identifier.\n\
             columns:\n\
             \x20 path: id integer NOT NULL\n\
             \x20 response 200 Pet\n\
             references (outgoing):\n\
             \x20 -> Pet ($ref)\n\
             \x20 -> Error ($ref)"
        );
    }

    #[test]
    fn join_path_walks_relations_in_both_directions() {
        let shop = shop();
        assert_eq!(
            join_path(&shop, find(&shop, "order_items"), find(&shop, "customers")),
            "main.order_items -> main.customers in 2 joins:\n\
             \x20 main.order_items.order_id -> main.orders.id\n\
             \x20 main.orders.customer_id -> main.customers.id\n\
             \n\
             SELECT * FROM main.order_items JOIN main.orders ON main.order_items.order_id = main.orders.id JOIN main.customers ON main.orders.customer_id = main.customers.id"
        );
        // The same path backwards keeps each relation printed as stored.
        assert_eq!(
            join_path(&shop, find(&shop, "customers"), find(&shop, "order_items")),
            "main.customers -> main.order_items in 2 joins:\n\
             \x20 main.orders.customer_id -> main.customers.id\n\
             \x20 main.order_items.order_id -> main.orders.id\n\
             \n\
             SELECT * FROM main.customers JOIN main.orders ON main.orders.customer_id = main.customers.id JOIN main.order_items ON main.order_items.order_id = main.orders.id"
        );
    }

    #[test]
    fn join_path_handles_same_table_composite_keys_and_missing_paths() {
        let shop = shop();
        let orders = find(&shop, "orders");
        assert_eq!(join_path(&shop, orders, orders), "Same table.");
        assert_eq!(
            join_path(&shop, find(&shop, "order_totals"), orders),
            "sales.order_totals -> main.orders in 1 joins:\n\
             \x20 sales.order_totals -> main.orders (fk)\n\
             \n\
             SELECT * FROM sales.order_totals JOIN main.orders ON /* composite key: fk */"
        );
        let island = source(
            "island",
            "database",
            Graph {
                entities: vec![
                    table("main", "left", "table", vec![]),
                    table("main", "right", "table", vec![]),
                ],
                relations: vec![],
                warnings: vec![],
            },
        );
        assert_eq!(
            join_path(&island, find(&island, "left"), find(&island, "right")),
            "No foreign-key path between main.left and main.right within 6 joins."
        );
    }

    #[test]
    fn render_canvas_reports_positions_and_groups() {
        let mut placed = shop();
        placed
            .positions
            .insert(entity_id("main", "orders"), Position { x: 10.4, y: 20.6 });
        placed.positions.insert(
            entity_id("main", "customers"),
            Position { x: -0.2, y: 120.0 },
        );
        placed.groups.push(CanvasGroup {
            id: "g1".into(),
            name: "Core".into(),
            node_ids: vec![entity_id("main", "orders"), "missing".into()],
            color: "#879b91".into(),
        });
        assert_eq!(
            render_canvas(&placed),
            "2 of 4 nodes positioned; 1 groups; card width 284\n\
             nodes:\n\
             \x20 main.customers 0 120\n\
             \x20 main.orders 10 21\n\
             unpositioned: main.order_items, sales.order_totals\n\
             groups:\n\
             \x20 g1 \"Core\" #879b91: main.orders, missing"
        );
        let empty = shop();
        assert_eq!(
            render_canvas(&empty),
            "0 of 4 nodes positioned; 0 groups; card width 284\n\
             unpositioned: main.customers, main.order_items, main.orders, sales.order_totals"
        );
    }

    #[test]
    fn renders_the_bundled_commerce_catalog() {
        let graph: Graph =
            serde_json::from_str(include_str!("../../../examples/commerce-schema.json")).unwrap();
        let commerce = source("commerce", "database", graph);
        let rendered = render_schema(&commerce, None, 0, 0);
        assert_eq!(
            rendered.lines().next().unwrap(),
            "6 of 6 entities in commerce (offset 0)."
        );
        assert!(rendered.contains(
            "main.addresses (id INTEGER PK, customer_id INTEGER FK->main.customers.id NOT NULL,"
        ));
        assert!(rendered.contains("created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP"));
        let items = resolve_entity(&commerce, "order_items").unwrap();
        let customers = resolve_entity(&commerce, "main.customers").unwrap();
        let path = join_path(&commerce, items, customers);
        assert!(path.starts_with("main.order_items -> main.customers in 2 joins:"));
        assert!(path.contains("\n\nSELECT * FROM main.order_items JOIN main.orders ON"));
        assert!(describe(&commerce, customers).contains("referenced by (incoming):"));
    }
}
