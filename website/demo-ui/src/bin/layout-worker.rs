//! Worker entry point: no DOM, IPC, database access, or agent capabilities.
#[cfg(target_arch = "wasm32")]
fn main() {
    use schematlas_ui::{
        smart_layout::smart_layout,
        types::{CanvasGroup, Graph},
    };
    use wasm_bindgen::{prelude::*, JsCast};
    let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let sender = scope.clone();
    let callback =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let result = event
                .data()
                .as_string()
                .ok_or_else(|| "Invalid layout request".to_string())
                .and_then(|input| {
                    serde_json::from_str::<(Graph, Vec<CanvasGroup>)>(&input)
                        .map_err(|e| e.to_string())
                })
                .map(|(graph, groups)| smart_layout(&graph, &groups));
            // The outer Result distinguishes a failed layout from valid positions.
            if let Ok(message) = serde_json::to_string(&result) {
                let _ = sender.post_message(&message.into());
            }
        });
    scope.set_onmessage(Some(callback.as_ref().unchecked_ref()));
    // The worker owns this callback until termination; it never escapes to the main thread.
    callback.forget();
    let _ = scope.post_message(&JsValue::from_str("ready"));
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
