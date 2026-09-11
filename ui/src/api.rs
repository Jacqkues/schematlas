//! IPC boundary: Tauri commands and events on the desktop, a localStorage preview in the browser.
use crate::types::*;
use serde::{de::DeserializeOwned, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke, catch)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen, catch)]
    async fn tauri_listen(event: &str, handler: &JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"], js_name = save, catch)]
    async fn tauri_save(options: JsValue) -> Result<JsValue, JsValue>;
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "dialog"], js_name = open, catch)]
    async fn tauri_open(options: JsValue) -> Result<JsValue, JsValue>;
}

thread_local! {
    static DESKTOP: Cell<Option<bool>> = const { Cell::new(None) };
}

/// True inside the Tauri window, false in the visibly labeled browser preview.
pub fn desktop() -> bool {
    DESKTOP.with(|cell| {
        if let Some(value) = cell.get() {
            return value;
        }
        let value = web_sys::window()
            .map(|w| js_sys::Reflect::has(&w, &JsValue::from_str("__TAURI__")).unwrap_or(false))
            .unwrap_or(false);
        cell.set(Some(value));
        value
    })
}

pub fn js_error(value: JsValue) -> String {
    if let Some(text) = value.as_string() {
        return text;
    }
    if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        return String::from(error.message());
    }
    js_sys::JSON::stringify(&value)
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_else(|| "Unknown error".into())
}

fn to_js(value: &impl Serialize) -> Result<JsValue, String> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| e.to_string())
}

async fn call<T: DeserializeOwned>(command: &str, args: impl Serialize) -> Result<T, String> {
    if desktop() {
        let result = tauri_invoke(command, to_js(&args)?).await.map_err(js_error)?;
        serde_wasm_bindgen::from_value(result).map_err(|e| e.to_string())
    } else {
        let value = crate::preview::preview(command, serde_json::to_value(&args).map_err(|e| e.to_string())?)?;
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
}

async fn call_unit(command: &str, args: impl Serialize) -> Result<(), String> {
    if desktop() {
        tauri_invoke(command, to_js(&args)?).await.map_err(js_error)?;
    } else {
        crate::preview::preview(command, serde_json::to_value(&args).map_err(|e| e.to_string())?)?;
    }
    Ok(())
}

pub async fn list_projects() -> Result<Vec<Project>, String> {
    call("list_projects", serde_json::json!({})).await
}
pub async fn create_project(name: &str, description: &str) -> Result<Project, String> {
    call("create_project", serde_json::json!({ "name": name, "description": description })).await
}
pub async fn rename_project(project_id: &str, name: &str, description: &str) -> Result<Project, String> {
    call("rename_project", serde_json::json!({ "projectId": project_id, "name": name, "description": description })).await
}
pub async fn delete_project(project_id: &str) -> Result<(), String> {
    call_unit("delete_project", serde_json::json!({ "projectId": project_id })).await
}
pub async fn create_demo() -> Result<Project, String> {
    call("create_demo", serde_json::json!({})).await
}
pub async fn connect_database(
    project_id: &str,
    name: &str,
    kind: &str,
    connection_string: &str,
    source_id: Option<&str>,
) -> Result<Project, String> {
    call(
        "connect_database",
        serde_json::json!({
            "projectId": project_id,
            "name": name,
            "request": { "kind": kind, "connectionString": connection_string },
            "sourceId": source_id,
        }),
    )
    .await
}
pub async fn refresh_source(project_id: &str, source_id: &str) -> Result<Project, String> {
    call("refresh_source", serde_json::json!({ "projectId": project_id, "sourceId": source_id })).await
}
pub async fn import_openapi(project_id: &str, document: &str) -> Result<Project, String> {
    call("import_openapi", serde_json::json!({ "projectId": project_id, "document": document })).await
}
pub async fn remove_source(project_id: &str, source_id: &str) -> Result<Project, String> {
    call("remove_source", serde_json::json!({ "projectId": project_id, "sourceId": source_id })).await
}
pub async fn save_positions(project_id: &str, source_id: &str, positions: &HashMap<String, Position>) -> Result<Project, String> {
    call("save_positions", serde_json::json!({ "projectId": project_id, "sourceId": source_id, "positions": positions })).await
}
pub async fn export_source(project_id: &str, source_id: &str, path: &str) -> Result<(), String> {
    call_unit("export_source", serde_json::json!({ "projectId": project_id, "sourceId": source_id, "path": path })).await
}
pub async fn configure_api(project_id: &str, source_id: &str, base_url: &str, headers: serde_json::Value) -> Result<Project, String> {
    call(
        "configure_api",
        serde_json::json!({ "projectId": project_id, "sourceId": source_id, "config": { "baseUrl": base_url, "headers": headers } }),
    )
    .await
}
pub async fn set_group(
    project_id: &str,
    source_id: &str,
    group_id: Option<&str>,
    name: &str,
    color: &str,
    node_ids: &[String],
) -> Result<Project, String> {
    call(
        "set_canvas_group",
        serde_json::json!({
            "projectId": project_id,
            "args": { "source_id": source_id, "group_id": group_id, "name": name, "node_ids": node_ids, "color": color },
        }),
    )
    .await
}
pub async fn undo_canvas(project_id: &str, source_id: &str) -> Result<Project, String> {
    call("undo_canvas", serde_json::json!({ "projectId": project_id, "sourceId": source_id })).await
}
pub async fn remove_group(project_id: &str, source_id: &str, group_id: &str) -> Result<Project, String> {
    call("remove_canvas_group", serde_json::json!({ "projectId": project_id, "sourceId": source_id, "groupId": group_id })).await
}

// Local agent sessions (desktop only).
pub async fn agent_working_directory(project_id: &str) -> Result<String, String> {
    call("agent_working_directory", serde_json::json!({ "projectId": project_id })).await
}
pub async fn agent_status(project_id: &str) -> Result<Option<AgentSnapshot>, String> {
    call("agent_status", serde_json::json!({ "projectId": project_id })).await
}
pub async fn agent_connect(project_id: &str, executable: &str, args: &[String], cwd: &str) -> Result<(), String> {
    call_unit(
        "agent_connect",
        serde_json::json!({ "projectId": project_id, "config": { "executable": executable, "args": args, "cwd": cwd } }),
    )
    .await
}
pub async fn agent_prompt(project_id: &str, text: &str) -> Result<(), String> {
    call_unit("agent_prompt", serde_json::json!({ "projectId": project_id, "text": text })).await
}
pub async fn agent_cancel(project_id: &str) -> Result<(), String> {
    call_unit("agent_cancel", serde_json::json!({ "projectId": project_id })).await
}
pub async fn agent_disconnect(project_id: &str) -> Result<(), String> {
    call_unit("agent_disconnect", serde_json::json!({ "projectId": project_id })).await
}
pub async fn agent_decide(project_id: &str, review_id: &str, option: Option<&str>) -> Result<(), String> {
    call_unit("agent_decide", serde_json::json!({ "projectId": project_id, "reviewId": review_id, "option": option })).await
}
pub async fn agent_authenticate(project_id: &str, method_id: &str) -> Result<(), String> {
    call_unit("agent_authenticate", serde_json::json!({ "projectId": project_id, "methodId": method_id })).await
}
pub async fn discover_agents() -> Result<Vec<InstalledAgent>, String> {
    call("discover_agents", serde_json::json!({})).await
}

/// Native save dialog. Returns the chosen path, or None when cancelled.
pub async fn save_dialog(title: &str, default_path: &str, filter_name: &str, extensions: &[&str]) -> Result<Option<String>, String> {
    let options = to_js(&serde_json::json!({
        "title": title,
        "defaultPath": default_path,
        "filters": [{ "name": filter_name, "extensions": extensions }],
    }))?;
    let result = tauri_save(options).await.map_err(js_error)?;
    Ok(result.as_string())
}

/// Native open dialog for one file or directory. Returns the chosen path, or None when cancelled.
pub async fn open_dialog(title: &str, directory: bool, default_path: Option<&str>) -> Result<Option<String>, String> {
    let mut options = serde_json::json!({ "title": title, "directory": directory, "multiple": false });
    if let Some(path) = default_path.filter(|p| !p.is_empty()) {
        options["defaultPath"] = serde_json::Value::String(path.into());
    }
    let result = tauri_open(to_js(&options)?).await.map_err(js_error)?;
    Ok(result.as_string())
}

struct ListenerInner {
    closure: RefCell<Option<Closure<dyn FnMut(JsValue)>>>,
    unlisten: RefCell<Option<js_sys::Function>>,
    disposed: Cell<bool>,
}

impl ListenerInner {
    fn stop(&self) {
        if let Some(unlisten) = self.unlisten.borrow_mut().take() {
            let _ = unlisten.call0(&JsValue::NULL);
            self.closure.borrow_mut().take();
        }
    }
}

/// A Tauri event subscription that unsubscribes when dropped, even if registration is still pending.
pub struct Listener {
    inner: Rc<ListenerInner>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.inner.disposed.set(true);
        self.inner.stop();
    }
}

pub fn listen<T: DeserializeOwned + 'static>(event: &str, handler: impl Fn(T) + 'static) -> Listener {
    let inner = Rc::new(ListenerInner {
        closure: RefCell::new(None),
        unlisten: RefCell::new(None),
        disposed: Cell::new(false),
    });
    let listener = Listener { inner: inner.clone() };
    if !desktop() {
        return listener;
    }
    let callback = Closure::wrap(Box::new(move |raw: JsValue| {
        let payload = js_sys::Reflect::get(&raw, &JsValue::from_str("payload")).unwrap_or(JsValue::UNDEFINED);
        if let Ok(value) = serde_wasm_bindgen::from_value::<T>(payload) {
            handler(value);
        }
    }) as Box<dyn FnMut(JsValue)>);
    let handle: JsValue = callback.as_ref().clone();
    *inner.closure.borrow_mut() = Some(callback);
    let event = event.to_string();
    leptos::task::spawn_local(async move {
        match tauri_listen(&event, &handle).await {
            Ok(unlisten) => {
                *inner.unlisten.borrow_mut() = unlisten.dyn_into::<js_sys::Function>().ok();
                if inner.disposed.get() {
                    inner.stop();
                }
            }
            Err(_) => {
                inner.closure.borrow_mut().take();
            }
        }
    });
    listener
}

/// Browser fallback for exports: offer the JSON as a download.
pub fn download_json(file_name: &str, content: &str) -> Result<(), String> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or("No document")?;
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(content));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/json");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options).map_err(js_error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(js_error)?
        .dyn_into()
        .map_err(|_| "anchor".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(file_name);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
}

pub fn local_storage_get(key: &str) -> Option<String> {
    web_sys::window()?.local_storage().ok()??.get_item(key).ok()?
}

pub fn local_storage_set(key: &str, value: &str) {
    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = storage.set_item(key, value);
    }
}

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub fn now_iso() -> String {
    String::from(js_sys::Date::new_0().to_iso_string())
}
