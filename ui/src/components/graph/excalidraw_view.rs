//! `.excalidraw` scenes in the Excalidraw editor (React, bundled apart in `public/excalidraw/` and
//! loaded the first time one is opened). The editor reports the whole scene after each change;
//! the file on disk stays the source of truth and newer texts are pushed back into the editor.
use crate::api;
use crate::appearance::Appearance;
use crate::types::DiagramFile;
use js_sys::{Function, Object, Promise, Reflect};
use leptos::html::Div;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

const GLOBAL: &str = "SchematlasExcalidraw";

thread_local! {
    static LOADING: RefCell<Option<Promise>> = const { RefCell::new(None) };
}

fn global() -> Option<JsValue> {
    let window = web_sys::window()?;
    Reflect::get(&window, &GLOBAL.into())
        .ok()
        .filter(|g| !g.is_undefined())
}

/// Load the editor bundle and its stylesheet once; later calls wait for the same load.
async fn load() -> Result<JsValue, String> {
    if let Some(g) = global() {
        return Ok(g);
    }
    let promise = LOADING.with(|loading| {
        loading
            .borrow_mut()
            .get_or_insert_with(|| {
                Promise::new(&mut |resolve, reject| {
                    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
                        let _ = reject.call1(&JsValue::NULL, &"no document".into());
                        return;
                    };
                    let head = document.head();
                    if let Ok(link) = document.create_element("link") {
                        let _ = link.set_attribute("rel", "stylesheet");
                        let _ = link.set_attribute("href", "excalidraw/excalidraw.css");
                        if let Some(head) = &head {
                            let _ = head.append_child(&link);
                        }
                    }
                    let Ok(script) = document.create_element("script") else {
                        let _ = reject.call1(&JsValue::NULL, &"no script element".into());
                        return;
                    };
                    let _ = script.set_attribute("type", "module");
                    let _ = script.set_attribute("src", "excalidraw/excalidraw.js");
                    // A module script's load event fires once it has run.
                    let _ = script.add_event_listener_with_callback("load", &resolve);
                    let _ = script.add_event_listener_with_callback("error", &reject);
                    if let Some(head) = &head {
                        let _ = head.append_child(&script);
                    }
                })
            })
            .clone()
    });
    if JsFuture::from(promise).await.is_err() {
        LOADING.with(|loading| loading.borrow_mut().take());
        return Err("The Excalidraw editor could not be loaded.".into());
    }
    global().ok_or_else(|| "The Excalidraw editor did not start.".into())
}

fn call(target: &JsValue, method: &str, args: &[JsValue]) -> Result<JsValue, JsValue> {
    let f: Function = Reflect::get(target, &method.into())?.dyn_into()?;
    let array: js_sys::Array = args.iter().collect();
    f.apply(target, &array)
}

/// Saves one at a time, each against the hash the previous one left, so a burst of drawing
/// never conflicts with itself. Kept outside reactive state: the editor's last save can arrive
/// while the view is being torn down.
#[derive(Default)]
struct Saver {
    hash: String,
    busy: bool,
    pending: Option<String>,
    /// Texts this editor wrote lately: reloading one of them must not roll the drawing back.
    recent: std::collections::VecDeque<String>,
}

fn save(
    saver: Rc<RefCell<Saver>>,
    project_id: String,
    id: String,
    text: String,
    on_saved: Callback<Result<DiagramFile, String>>,
) {
    {
        let mut s = saver.borrow_mut();
        s.recent.push_back(text.clone());
        if s.recent.len() > 8 {
            s.recent.pop_front();
        }
        if s.busy {
            s.pending = Some(text);
            return;
        }
        s.busy = true;
    }
    spawn_local(async move {
        let mut text = text;
        loop {
            let hash = saver.borrow().hash.clone();
            let result = api::restore_diagram(&project_id, &id, &text, &hash).await;
            let next = {
                let mut s = saver.borrow_mut();
                if let Ok(file) = &result {
                    s.hash = file.hash.clone();
                }
                let next = s.pending.take();
                s.busy = next.is_some() && result.is_ok();
                if result.is_err() {
                    s.pending = None;
                }
                next
            };
            let ok = result.is_ok();
            let _ = on_saved.try_run(result);
            match next {
                Some(n) if ok => text = n,
                _ => break,
            }
        }
    });
}

/// The scene `text` in an editor, saved to `id` as the drawing settles. `on_saved` receives
/// each write's result; `on_select` the single shape selected, if any.
#[component]
pub fn ExcalidrawView(
    project_id: String,
    id: String,
    #[prop(into)] text: Signal<String>,
    #[prop(into)] hash: Signal<String>,
    #[prop(into)] on_saved: Callback<Result<DiagramFile, String>>,
    #[prop(into)] on_select: Callback<Option<String>>,
) -> impl IntoView {
    let host = NodeRef::<Div>::new();
    let editor = StoredValue::new_local(None::<JsValue>);
    // The callbacks the editor holds, dropped after it is unmounted.
    type Callbacks = (Closure<dyn Fn(String)>, Closure<dyn Fn(JsValue)>);
    let callbacks = StoredValue::new_local(None::<Callbacks>);
    let error = RwSignal::new(String::new());
    let saver = Rc::new(RefCell::new(Saver::default()));
    {
        let saver = saver.clone();
        Effect::new(move |_| {
            let hash = hash.get();
            let mut s = saver.borrow_mut();
            if !s.busy {
                s.hash = hash;
            }
        });
    }
    let recent = saver.clone();
    let light = use_context::<Appearance>().map(|a| a.0);
    let theme = move || {
        if light.is_some_and(|l| l.get()) {
            "light"
        } else {
            "dark"
        }
    };

    Effect::new(move |_| {
        let Some(element) = host.get() else {
            return;
        };
        if editor.with_value(Option::is_some) {
            return;
        }
        let initial = text.get_untracked();
        let theme = theme();
        let (saver, project_id, id) = (saver.clone(), project_id.clone(), id.clone());
        spawn_local(async move {
            let api = match load().await {
                Ok(api) => api,
                Err(e) => {
                    error.try_set(e);
                    return;
                }
            };
            let (saver, project_id, id) = (saver.clone(), project_id.clone(), id.clone());
            let save = Closure::<dyn Fn(String)>::new(move |next: String| {
                save(
                    saver.clone(),
                    project_id.clone(),
                    id.clone(),
                    next,
                    on_saved,
                );
            });
            let select = Closure::<dyn Fn(JsValue)>::new(move |node: JsValue| {
                let _ = on_select.try_run(node.as_string());
            });
            let options = Object::new();
            let _ = Reflect::set(&options, &"text".into(), &initial.into());
            let _ = Reflect::set(&options, &"theme".into(), &theme.into());
            let _ = Reflect::set(&options, &"onSave".into(), save.as_ref());
            let _ = Reflect::set(&options, &"onSelect".into(), select.as_ref());
            match call(&api, "mount", &[element.into(), options.into()]) {
                Ok(handle) => {
                    // The view may have closed while the bundle was loading.
                    if editor.try_set_value(Some(handle.clone())).is_none() {
                        callbacks.set_value(Some((save, select)));
                    } else {
                        let _ = call(&handle, "unmount", &[]);
                    }
                }
                Err(_) => {
                    error.try_set("The Excalidraw editor could not open this file.".into());
                }
            }
        });
    });
    // Newer texts of the file (an agent's annotation, git, another editor) replace the scene.
    Effect::new(move |_| {
        let next = text.get();
        if recent.borrow().recent.contains(&next) {
            return;
        }
        editor.with_value(|e| {
            if let Some(e) = e {
                let _ = call(e, "load", &[next.into()]);
            }
        });
    });
    Effect::new(move |_| {
        let theme = theme();
        editor.with_value(|e| {
            if let Some(e) = e {
                let _ = call(e, "setTheme", &[theme.into()]);
            }
        });
    });
    on_cleanup(move || {
        if let Some(Some(e)) = editor.try_update_value(Option::take) {
            let _ = call(&e, "unmount", &[]);
        }
        callbacks.try_update_value(Option::take);
    });

    view! {
        <div class="relative min-h-0 min-w-0 flex-1" aria-label="Excalidraw editor" role="region">
            <div node_ref=host class="excalidraw-host absolute inset-0"></div>
            <Show when=move || !error.get().is_empty()>
                <p class="absolute inset-0 m-auto h-fit max-w-[420px] p-6 text-center text-xs text-danger-text" role="alert">{move || error.get()}</p>
            </Show>
        </div>
    }
}
