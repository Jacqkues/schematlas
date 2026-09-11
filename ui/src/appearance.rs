//! Shared, persisted appearance; the HTML bootstrap applies it before WASM starts.
use crate::api;
use leptos::prelude::*;

const KEY: &str = "schematlas.theme";

#[derive(Clone, Copy)]
pub struct Appearance(pub RwSignal<bool>);

impl Appearance {
    pub fn new() -> Self {
        let theme = Self(RwSignal::new(
            api::local_storage_get(KEY).as_deref() == Some("light"),
        ));
        theme.apply();
        theme
    }

    pub fn toggle(self) {
        self.0.update(|light| *light = !*light);
        self.apply();
    }

    fn apply(self) {
        let light = self.0.get_untracked();
        let theme = if light { "light" } else { "dark" };
        let color = if light { "#f7f8fa" } else { "#070809" };
        api::local_storage_set(KEY, theme);
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Some(root) = document.document_element() {
                let _ = root.set_attribute("data-theme", theme);
            }
            for (name, value) in [("color-scheme", theme), ("theme-color", color)] {
                if let Ok(Some(meta)) = document.query_selector(&format!("meta[name='{name}']")) {
                    let _ = meta.set_attribute("content", value);
                }
            }
        }
        if api::desktop() {
            leptos::task::spawn_local(async move {
                let _ = api::set_window_theme(theme, color).await;
            });
        }
    }
}
