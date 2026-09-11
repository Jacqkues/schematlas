use crate::{
    domain::Source,
    error::{AppError, Result},
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Duration};
use url::Url;
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiConnection {
    pub base_url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
}
#[derive(Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct HttpRequest {
    pub source_id: String,
    /// Exact operation entity id returned by get_schema.
    pub operation_id: String,
    #[serde(default)]
    pub path_parameters: BTreeMap<String, String>,
    #[serde(default)]
    pub query: BTreeMap<String, String>,
    pub body: Option<Value>,
}
pub fn validate_config(config: &ApiConnection) -> Result<Url> {
    let url = Url::parse(&config.base_url).map_err(|_| {
        AppError::Validation("Enter an absolute HTTP or HTTPS API base URL.".into())
    })?;
    if !["http", "https"].contains(&url.scheme())
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(AppError::Validation(
            "Use an HTTP(S) base URL without credentials, query parameters, or fragment.".into(),
        ));
    }
    if config.headers.len() > 30 {
        return Err(AppError::Validation("Use at most 30 API headers.".into()));
    }
    for (name, value) in &config.headers {
        if [
            "host",
            "connection",
            "content-length",
            "transfer-encoding",
            "proxy-authorization",
        ]
        .contains(&name.to_ascii_lowercase().as_str())
            || reqwest::header::HeaderName::from_bytes(name.as_bytes()).is_err()
            || reqwest::header::HeaderValue::from_str(value).is_err()
        {
            return Err(AppError::Validation(
                "An API header is invalid or reserved.".into(),
            ));
        }
    }
    Ok(url)
}
pub fn resolve(
    source: &Source,
    config: &ApiConnection,
    request: &HttpRequest,
) -> Result<(String, Url)> {
    if source.kind != "openapi" || source.id != request.source_id {
        return Err(AppError::NotFound);
    }
    let operation = source
        .graph
        .entities
        .iter()
        .find(|e| e.id == request.operation_id && e.kind == "operation")
        .ok_or(AppError::NotFound)?;
    let method = operation.method.clone().ok_or(AppError::NotFound)?;
    let mut route = operation.name.clone();
    for (key, value) in &request.path_parameters {
        if value == "." || value == ".." || value.len() > 4096 {
            return Err(AppError::Validation("Invalid path parameter.".into()));
        }
        route = route.replace(
            &format!("{{{key}}}"),
            &percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC)
                .to_string(),
        );
    }
    if route.contains(['{', '}'])
        || !route.starts_with('/')
        || route.contains(['?', '#', '\\'])
        || route.split('/').any(|s| s == ".." || s == ".")
    {
        return Err(AppError::Validation(
            "Provide every path parameter for the selected API operation.".into(),
        ));
    }
    let mut url = validate_config(config)?;
    url.set_path(&format!("{}{}", url.path().trim_end_matches('/'), route));
    if !request.query.is_empty() {
        url.query_pairs_mut().extend_pairs(&request.query);
    }
    if request
        .body
        .as_ref()
        .is_some_and(|v| v.to_string().len() > 1024 * 1024)
    {
        return Err(AppError::Validation(
            "Request bodies must be at most 1 MB.".into(),
        ));
    }
    Ok((method, url))
}
pub async fn execute(
    source: &Source,
    config: &ApiConnection,
    request: &HttpRequest,
) -> Result<Value> {
    let (method, url) = resolve(source, config, request)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| AppError::Validation("Could not initialize HTTP client.".into()))?;
    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|_| AppError::Validation("Invalid HTTP method.".into()))?;
    let mut builder = client.request(method, url);
    for (name, value) in &config.headers {
        builder = builder.header(name, value);
    }
    if let Some(body) = &request.body {
        builder = builder.json(body);
    }
    let response = builder.send().await.map_err(|_| {
        AppError::Validation(
            "API request failed. Check the configured address, network, TLS, and authentication."
                .into(),
        )
    })?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    let mut truncated = false;
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|_| AppError::Validation("Could not read the API response.".into()))?;
        let left = (1024 * 1024usize).saturating_sub(bytes.len());
        if chunk.len() > left {
            bytes.extend_from_slice(&chunk[..left]);
            truncated = true;
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = if truncated {
        json!(String::from_utf8_lossy(&bytes))
    } else {
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)))
    };
    Ok(json!({"status":status,"contentType":content_type,"body":body,"truncated":truncated}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_configured_origin_and_documented_operation() {
        let (_, graph) =
            crate::openapi::parse(include_str!("../../../examples/commerce.openapi.json")).unwrap();
        let source = Source {
            groups: vec![],
            layout_backup: None,
            id: "api".into(),
            name: "API".into(),
            kind: "openapi".into(),
            database_kind: None,
            imported_at: String::new(),
            graph,
            positions: BTreeMap::new(),
            api_base_url: None,
        };
        let config = ApiConnection {
            base_url: "http://localhost:8080/v1".into(),
            headers: BTreeMap::new(),
        };
        let request = HttpRequest {
            source_id: "api".into(),
            operation_id: "operation:get:/customers/{id}".into(),
            path_parameters: BTreeMap::from([("id".into(), "alice/bob".into())]),
            query: BTreeMap::new(),
            body: None,
        };
        let (_, url) = resolve(&source, &config, &request).unwrap();
        assert_eq!(
            url.as_str(),
            "http://localhost:8080/v1/customers/alice%2Fbob"
        );
        let mut bad = request;
        bad.path_parameters.insert("id".into(), "..".into());
        assert!(resolve(&source, &config, &bad).is_err());
        assert!(validate_config(&ApiConnection {
            base_url: "https://user:pass@example.test".into(),
            headers: BTreeMap::new()
        })
        .is_err());
    }
}
