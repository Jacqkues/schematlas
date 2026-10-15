//! Asynchronous boundary to the isolated graph layout worker.
use crate::types::{CanvasGroup, Graph, Position};
use schematlas_ui::flow_layout::{FlowInput, FlowLayout, Job, Route};
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

async fn ask<T: serde::de::DeserializeOwned>(
    job: &Job,
    signal: &web_sys::AbortSignal,
) -> Result<T, String> {
    let input = serde_json::to_string(job).map_err(|e| e.to_string())?;
    let result = layout_in_worker(&input, signal)
        .await
        .map_err(crate::api::js_error)?;
    let json = result.as_string().ok_or("Invalid layout response")?;
    serde_json::from_str::<Result<T, String>>(&json).map_err(|e| e.to_string())?
}

pub async fn arrange(
    graph: &Graph,
    groups: &[CanvasGroup],
    signal: &web_sys::AbortSignal,
) -> Result<HashMap<String, Position>, String> {
    ask(&Job::Schema(graph.clone(), groups.to_vec()), signal).await
}

/// Lay out a diagram off the main thread.
pub async fn flow(input: FlowInput, signal: &web_sys::AbortSignal) -> Result<FlowLayout, String> {
    ask(&Job::Flow(input), signal).await
}

/// Edge routes for a hand-arranged diagram, off the main thread.
pub async fn route(
    input: FlowInput,
    positions: HashMap<String, Position>,
    signal: &web_sys::AbortSignal,
) -> Result<Vec<Option<Route>>, String> {
    ask(&Job::Route(input, positions), signal).await
}
