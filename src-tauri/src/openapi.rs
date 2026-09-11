//! Converts OpenAPI documents into a finite graph. References become edges;
//! recursive schemas are never recursively expanded into nodes.
use crate::{
    domain::{Entity, Field, Graph, Relation},
    error::{AppError, Result},
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashSet};
pub const MAX_DOCUMENT_BYTES: usize = 20 * 1024 * 1024;
const METHODS: [&str; 8] = [
    "get", "post", "put", "patch", "delete", "head", "options", "trace",
];

pub fn parse(text: &str) -> Result<(String, Graph)> {
    if text.len() > MAX_DOCUMENT_BYTES {
        return Err(AppError::Validation(
            "OpenAPI files must be smaller than 20 MB.".into(),
        ));
    }
    let doc: Value = serde_json::from_str(text)?;
    let version = doc
        .get("openapi")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let swagger = doc.get("swagger").and_then(Value::as_str) == Some("2.0");
    if !version.starts_with("3.") && !swagger {
        return Err(AppError::Validation(
            "Expected an OpenAPI 3.x or Swagger 2.0 JSON document.".into(),
        ));
    }
    let title = doc
        .pointer("/info/title")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| AppError::Validation("OpenAPI info.title is required.".into()))?
        .to_owned();
    if !doc.get("paths").is_some_and(Value::is_object)
        && !doc
            .pointer("/components/schemas")
            .is_some_and(Value::is_object)
    {
        return Err(AppError::Validation(
            "The document must contain a paths or components.schemas object.".into(),
        ));
    }
    let prefix = if swagger {
        "#/definitions/"
    } else {
        "#/components/schemas/"
    };
    let definitions = if swagger {
        doc.get("definitions")
    } else {
        doc.pointer("/components/schemas")
    };
    let mut graph = Graph::default();
    let mut references: Vec<(String, Value)> = vec![];
    if let Some(schemas) = definitions.and_then(Value::as_object) {
        for (name, schema) in schemas {
            let id = format!("{prefix}{}", escape_pointer(name));
            let mut properties = BTreeMap::new();
            let mut required = BTreeSet::new();
            collect_properties(
                schema,
                &doc,
                &mut HashSet::new(),
                &mut properties,
                &mut required,
                0,
            );
            let mut fields: Vec<Field> = properties
                .into_iter()
                .map(|(name, value)| field(&name, &value, required.contains(&name)))
                .collect();
            if fields.is_empty() {
                fields.push(field("value", schema, false));
            }
            graph.entities.push(Entity {
                id: id.clone(),
                name: name.clone(),
                namespace: "Schemas".into(),
                kind: "schema".into(),
                description: description(schema),
                fields,
                method: None,
                unique_keys: None,
            });
            references.push((id, schema.clone()));
        }
    }
    if let Some(paths) = doc.get("paths").and_then(Value::as_object) {
        for (path, item) in paths {
            if !path.starts_with('/') {
                continue;
            }
            let item = resolve(item, &doc);
            for method in METHODS {
                let Some(operation) = item.get(method).filter(|v| v.is_object()) else {
                    continue;
                };
                let id = format!("operation:{method}:{path}");
                let mut params: BTreeMap<(String, String), Value> = BTreeMap::new();
                for container in [&item, operation] {
                    if let Some(parameters) = container.get("parameters").and_then(Value::as_array)
                    {
                        for parameter in parameters {
                            let p = resolve(parameter, &doc);
                            params.insert((string(&p, "in"), string(&p, "name")), p);
                        }
                    }
                }
                let mut fields = vec![];
                for ((location, name), p) in params {
                    let schema = p.get("schema").unwrap_or(&p);
                    let mut f = field(
                        &format!("{location}: {name}"),
                        schema,
                        p.get("required") == Some(&Value::Bool(true)) || location == "path",
                    );
                    f.description = description(&p);
                    fields.push(f);
                }
                if let Some(body) = operation.get("requestBody") {
                    let body = resolve(body, &doc);
                    fields.push(Field {
                        name: "request body".into(),
                        data_type: content_type(&body),
                        required: body.get("required") == Some(&Value::Bool(true)),
                        description: description(&body),
                        ..Field::default()
                    });
                }
                if let Some(responses) = operation.get("responses").and_then(Value::as_object) {
                    for (code, response) in responses {
                        let response = resolve(response, &doc);
                        fields.push(Field {
                            name: format!("response {code}"),
                            data_type: if let Some(s) = response.get("schema") {
                                schema_type(s, 0)
                            } else {
                                content_type(&response)
                            },
                            description: description(&response),
                            ..Field::default()
                        });
                    }
                }
                let namespace = operation
                    .get("tags")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .and_then(Value::as_str)
                    .unwrap_or("Endpoints")
                    .to_owned();
                let desc = match (
                    operation.get("summary").and_then(Value::as_str),
                    description(operation),
                ) {
                    (Some(s), Some(d)) => Some(format!("{s}\n\n{d}")),
                    (Some(s), None) => Some(s.into()),
                    (_, d) => d,
                };
                graph.entities.push(Entity {
                    id: id.clone(),
                    name: path.clone(),
                    namespace,
                    kind: "operation".into(),
                    description: desc,
                    fields,
                    method: Some(method.to_uppercase()),
                    unique_keys: None,
                });
                let inherited = item.get("parameters").cloned().unwrap_or(Value::Null);
                references.push((id, serde_json::json!([operation, inherited])));
            }
        }
    }
    let ids: HashSet<_> = graph.entities.iter().map(|e| e.id.clone()).collect();
    let mut warnings = BTreeSet::new();
    for (source, value) in references {
        let mut targets = BTreeSet::new();
        collect_references(
            &value,
            &doc,
            prefix,
            &mut HashSet::new(),
            &mut targets,
            &mut warnings,
            0,
        );
        for target in targets {
            if ids.contains(&target) {
                graph.relations.push(Relation {
                    id: format!("reference:{}", graph.relations.len()),
                    source: source.clone(),
                    target,
                    source_field: None,
                    target_field: None,
                    label: "$ref".into(),
                });
            } else {
                warnings.insert(format!("Unresolved schema reference: {target}"));
            }
        }
    }
    if doc.get("webhooks").is_some() {
        warnings.insert("Top-level webhooks are not included in this endpoint view.".into());
    }
    graph.warnings = warnings.into_iter().collect();
    graph.validate()?;
    Ok((title, graph))
}
fn string(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn description(v: &Value) -> Option<String> {
    v.get("description")
        .and_then(Value::as_str)
        .map(str::to_owned)
}
fn escape_pointer(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}
fn resolve(v: &Value, doc: &Value) -> Value {
    let mut current = v;
    let mut visited = HashSet::new();
    while let Some(r) = current.get("$ref").and_then(Value::as_str) {
        if !r.starts_with("#/") || !visited.insert(r) {
            break;
        }
        let Some(next) = doc.pointer(&r[1..]) else {
            break;
        };
        current = next;
    }
    current.clone()
}
fn field(name: &str, v: &Value, required: bool) -> Field {
    let nullable = v.get("nullable") == Some(&Value::Bool(true))
        || v.get("type")
            .and_then(Value::as_array)
            .is_some_and(|a| a.iter().any(|t| t == "null"));
    Field {
        name: name.into(),
        data_type: schema_type(v, 0),
        nullable,
        primary_key: false,
        required,
        default_value: v.get("default").map(Value::to_string),
        description: description(v),
    }
}
fn schema_type(v: &Value, depth: usize) -> String {
    if depth > 24 {
        return "…".into();
    }
    if let Some(r) = v.get("$ref").and_then(Value::as_str) {
        return r
            .rsplit('/')
            .next()
            .unwrap_or(r)
            .replace("~1", "/")
            .replace("~0", "~");
    }
    if let Some(values) = v.get("enum").and_then(Value::as_array) {
        return format!("enum ({})", values.len());
    }
    if let Some(t) = v.get("type").and_then(Value::as_array) {
        return t
            .iter()
            .map(|v| v.as_str().unwrap_or("unknown"))
            .collect::<Vec<_>>()
            .join(" | ");
    }
    if v.get("type").and_then(Value::as_str) == Some("array") {
        return format!("{}[]", schema_type(&v["items"], depth + 1));
    }
    for key in ["oneOf", "anyOf", "allOf"] {
        if let Some(a) = v.get(key).and_then(Value::as_array) {
            return a
                .iter()
                .map(|s| schema_type(s, depth + 1))
                .collect::<Vec<_>>()
                .join(if key == "allOf" { " & " } else { " | " });
        }
    }
    let t = v
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or(if v.get("properties").is_some() {
            "object"
        } else {
            "any"
        });
    if let Some(f) = v.get("format").and_then(Value::as_str) {
        format!("{t} · {f}")
    } else {
        t.into()
    }
}
fn content_type(body: &Value) -> String {
    body.get("content")
        .and_then(Value::as_object)
        .map(|content| {
            content
                .iter()
                .map(|(mime, v)| {
                    v.get("schema")
                        .map(|s| schema_type(s, 0))
                        .unwrap_or_else(|| mime.clone())
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "—".into())
}
fn collect_properties(
    v: &Value,
    doc: &Value,
    visited: &mut HashSet<String>,
    props: &mut BTreeMap<String, Value>,
    required: &mut BTreeSet<String>,
    depth: usize,
) {
    if depth > 32 {
        return;
    }
    if let Some(r) = v.get("$ref").and_then(Value::as_str) {
        if r.starts_with("#/") && visited.insert(r.into()) {
            if let Some(next) = doc.pointer(&r[1..]) {
                collect_properties(next, doc, visited, props, required, depth + 1);
            }
        }
    }
    if let Some(a) = v.get("allOf").and_then(Value::as_array) {
        for s in a {
            collect_properties(s, doc, visited, props, required, depth + 1);
        }
    }
    if let Some(p) = v.get("properties").and_then(Value::as_object) {
        props.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    if let Some(a) = v.get("required").and_then(Value::as_array) {
        required.extend(a.iter().filter_map(Value::as_str).map(str::to_owned));
    }
}
fn collect_references(
    v: &Value,
    doc: &Value,
    prefix: &str,
    visited: &mut HashSet<String>,
    targets: &mut BTreeSet<String>,
    warnings: &mut BTreeSet<String>,
    depth: usize,
) {
    if depth > 64 {
        warnings.insert("Some references exceeded the traversal depth limit (64).".into());
        return;
    }
    match v {
        Value::Object(map) => {
            if let Some(r) = map.get("$ref").and_then(Value::as_str) {
                if !r.starts_with("#/") {
                    warnings.insert(format!("External reference was not loaded: {r}"));
                } else if r.starts_with(prefix) && !r[prefix.len()..].contains('/') {
                    targets.insert(r.into());
                } else if visited.insert(r.into()) {
                    if let Some(next) = doc.pointer(&r[1..]) {
                        collect_references(
                            next,
                            doc,
                            prefix,
                            visited,
                            targets,
                            warnings,
                            depth + 1,
                        );
                    } else {
                        warnings.insert(format!("Unresolved reference: {r}"));
                    }
                }
            }
            for (key, value) in map {
                if !["example", "examples", "default", "enum"].contains(&key.as_str()) {
                    collect_references(value, doc, prefix, visited, targets, warnings, depth + 1);
                }
            }
        }
        Value::Array(a) => {
            for item in a {
                collect_references(item, doc, prefix, visited, targets, warnings, depth + 1);
            }
        }
        _ => {}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_cycles_composition_escaped_refs_and_shared_responses() {
        let doc = serde_json::json!({"openapi":"3.1.0","info":{"title":"Example"},"paths":{"/users":{"get":{"responses":{"200":{"$ref":"#/components/responses/Users"}}}}},"components":{"responses":{"Users":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/User~1Profile"}}}}},"schemas":{"User/Profile":{"allOf":[{"$ref":"#/components/schemas/Base"},{"type":"object","required":["friend"],"properties":{"friend":{"$ref":"#/components/schemas/User~1Profile"}}}]},"Base":{"type":"object","properties":{"id":{"type":"integer"}}}}}});
        let (_, g) = parse(&doc.to_string()).unwrap();
        assert_eq!(g.entities.len(), 3);
        assert_eq!(g.relations.len(), 3);
        let user = g
            .entities
            .iter()
            .find(|e| e.name == "User/Profile")
            .unwrap();
        assert_eq!(user.fields.len(), 2);
        assert!(
            user.fields
                .iter()
                .find(|f| f.name == "friend")
                .unwrap()
                .required
        );
        assert!(g.warnings.is_empty());
        g.validate().unwrap();
    }
    #[test]
    fn rejects_invalid_docs_and_reports_external_refs() {
        assert!(parse("{}").is_err());
        assert!(parse("{bad").is_err());
        let (_,g)=parse(r##"{"swagger":"2.0","info":{"title":"API"},"paths":{},"definitions":{"A":{"$ref":"https://example.test/model.json"}}}"##).unwrap();
        assert_eq!(g.warnings.len(), 1);
    }
    #[test]
    fn inherited_parameters_and_nullable_unions_are_visible() {
        let (_,g)=parse(r##"{"openapi":"3.1.0","info":{"title":"API"},"paths":{"/x/{id}":{"parameters":[{"name":"id","in":"path","schema":{"type":"string"}}],"get":{"responses":{"204":{"description":"Empty"}}}}},"components":{"schemas":{"X":{"properties":{"name":{"type":["string","null"]}}}}}}"##).unwrap();
        assert!(
            g.entities
                .iter()
                .find(|e| e.kind == "operation")
                .unwrap()
                .fields[0]
                .required
        );
        assert!(
            g.entities
                .iter()
                .find(|e| e.kind == "schema")
                .unwrap()
                .fields[0]
                .nullable
        );
    }
}
