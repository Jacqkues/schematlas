//! Asynchronous boundary to the isolated graph layout worker.
use crate::types::{CanvasGroup, Graph, Position};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/src/layout-worker-client.js")]
extern "C" {
    #[wasm_bindgen(js_name = layoutInWorker, catch)]
    async fn layout_in_worker(
        input: &str,
        signal: &web_sys::AbortSignal,
    ) -> Result<JsValue, JsValue>;
}

pub async fn arrange(
    graph: &Graph,
    groups: &[CanvasGroup],
    signal: &web_sys::AbortSignal,
) -> Result<HashMap<String, Position>, String> {
    let input = serde_json::to_string(&(graph, groups)).map_err(|e| e.to_string())?;
    let result = layout_in_worker(&input, signal)
        .await
        .map_err(crate::api::js_error)?;
    let json = result.as_string().ok_or("Invalid layout response")?;
    serde_json::from_str::<Result<HashMap<String, Position>, String>>(&json)
        .map_err(|e| e.to_string())?
}
