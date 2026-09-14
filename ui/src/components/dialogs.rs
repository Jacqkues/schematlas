//! Native dialogs: the shared modal frame and the project, connection, import, confirm and API forms.
use super::icons::Icon;
use crate::api;
use crate::types::{database_name, Project, Source, DATABASE_KINDS};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use wasm_bindgen::JsCast;

/// A confirmable async step; the dialog shows its busy and error states.
pub type Action = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>>>> + Send + Sync>;

pub fn action<F, Fut>(f: F) -> Action
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<(), String>> + 'static,
{
    Arc::new(move || Box::pin(f()))
}

#[component]
pub fn Modal(
    #[prop(into)] title: Signal<String>,
    #[prop(optional, into)] subtitle: Signal<String>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(optional, into)] busy: Signal<bool>,
    children: Children,
) -> impl IntoView {
    let dialog = NodeRef::<leptos::html::Dialog>::new();
    Effect::new(move |_| {
        if let Some(element) = dialog.get() {
            if !element.open() {
                let _ = element.show_modal();
            }
        }
    });
    view! {
        <dialog
            node_ref=dialog
            class="m-auto max-h-[90dvh] w-[560px] max-w-[calc(100vw-48px)] overflow-auto rounded-[13px] border border-line bg-surface p-[30px] text-ink shadow-[0_30px_120px_#0009] open:animate-dialog-in backdrop:bg-overlay backdrop:backdrop-blur-[4px] open:backdrop:animate-fade-in"
            aria-labelledby="modal-title"
            on:cancel=move |ev: leptos::ev::Event| {
                ev.prevent_default();
                if !busy.get_untracked() {
                    on_close.run(());
                }
            }
        >
            <div class="mb-[27px] flex items-start justify-between gap-2.5">
                <div>
                    <span class="eyebrow">"SCHEMATLAS"</span>
                    <h2 id="modal-title" class="mt-2 text-[25px] font-medium tracking-[-0.8px]">{move || title.get()}</h2>
                    <Show when=move || !subtitle.get().is_empty()>
                        <p class="mt-[9px] text-xs leading-[1.8] text-muted">{move || subtitle.get()}</p>
                    </Show>
                </div>
                <button
                    type="button"
                    class="icon-btn -mt-2.5 -mr-2.5"
                    aria-label="Close dialog"
                    disabled=move || busy.get()
                    on:click=move |_| on_close.run(())
                >
                    <Icon name="x" size=19 />
                </button>
            </div>
            {children()}
        </dialog>
    }
}

#[component]
pub fn FormError(#[prop(into)] error: Signal<String>) -> impl IntoView {
    view! {
        <Show when=move || !error.get().is_empty()>
            <p role="alert" class="form-error">{move || error.get()}</p>
        </Show>
    }
}

#[component]
pub fn ProjectDialog(
    project: Option<Arc<Project>>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_save: Callback<Project>,
    #[prop(into)] on_delete: Callback<()>,
) -> impl IntoView {
    let editing = project.is_some();
    let project_id = project.as_ref().map(|p| p.id.clone());
    let name = RwSignal::new(project.as_ref().map(|p| p.name.clone()).unwrap_or_default());
    let description = RwSignal::new(
        project
            .as_ref()
            .map(|p| p.description.clone())
            .unwrap_or_default(),
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let project_id = project_id.clone();
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let (n, d) = (name.get_untracked(), description.get_untracked());
            let result = match &project_id {
                Some(id) => api::rename_project(id, &n, &d).await,
                None => api::create_project(&n, &d).await,
            };
            match result {
                Ok(p) => {
                    on_save.run(p);
                    on_close.run(());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    view! {
        <Modal
            title=if editing { "Project settings" } else { "A place for your architecture." }
            subtitle=if editing { "Keep your workspace organized." } else { "Give your databases and APIs a shared home." }
            on_close=on_close
            busy=busy
        >
            <form on:submit=submit>
                <label class="form-label" for="project-name">"Project name " <span class="text-accent">"*"</span></label>
                <input id="project-name" class="field mb-[21px]" name="name" bind:value=name placeholder="e.g. Customer platform" required maxlength="80" />
                <label class="form-label" for="project-description">
                    "Description " <small class="float-right text-[10px] font-normal text-muted">"Optional"</small>
                </label>
                <textarea id="project-description" class="field mb-[21px] max-h-60 min-h-[90px] resize-y" name="description" bind:value=description placeholder="What are you mapping?" maxlength="2000" rows="3"></textarea>
                <FormError error=error />
                <div class="modal-footer">
                    {if editing {
                        view! {
                            <button class="btn btn-ghost-danger" type="button" on:click=move |_| on_delete.run(()) disabled=move || busy.get()>
                                <Icon name="trash-2" size=15 /> " Delete project"
                            </button>
                        }.into_any()
                    } else {
                        view! { <span class="form-hint">"Saved locally on this computer."</span> }.into_any()
                    }}
                    <button type="submit" class="btn btn-primary" disabled=move || busy.get()>
                        {move || if busy.get() { "Saving…" } else if editing { "Save changes" } else { "Create project" }}
                        <Icon name="arrow-right" size=16 />
                    </button>
                </div>
            </form>
        </Modal>
    }
}

fn placeholder(kind: &str) -> &'static str {
    match kind {
        "postgres" => "postgresql://user:password@localhost:5432/database?sslmode=require",
        "mysql" | "mariadb" => "mysql://user:password@localhost:3306/database?ssl-mode=REQUIRED",
        "sqlite" => "/Users/you/data/database.sqlite",
        _ => "Server=tcp:localhost,1433;Database=app;User ID=sa;Password=…;Encrypt=true",
    }
}

#[component]
pub fn ConnectDialog(
    project_id: String,
    source: Option<Arc<Source>>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_save: Callback<Project>,
) -> impl IntoView {
    let reconnect = source.is_some();
    let source_id = source.as_ref().map(|s| s.id.clone());
    let kind = RwSignal::new(
        source
            .as_ref()
            .and_then(|s| s.database_kind.clone())
            .unwrap_or_else(|| "postgres".into()),
    );
    let name = RwSignal::new(source.as_ref().map(|s| s.name.clone()).unwrap_or_default());
    let dsn = RwSignal::new(String::new());
    let reveal = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let sqlite = move || kind.get() == "sqlite";
    let browse = move |_| {
        spawn_local(async move {
            if !api::desktop() {
                error.set("File selection is available in the desktop app.".into());
                return;
            }
            match api::open_dialog("Choose SQLite database", false, None).await {
                Ok(Some(path)) => dsn.set(path),
                Ok(None) => {}
                Err(e) => error.set(e),
            }
        });
    };
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (project_id, source_id) = (project_id.clone(), source_id.clone());
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let result = api::connect_database(
                &project_id,
                &name.get_untracked(),
                &kind.get_untracked(),
                dsn.get_untracked().trim(),
                source_id.as_deref(),
            )
            .await;
            match result {
                Ok(p) => {
                    dsn.set(String::new());
                    on_save.run(p);
                    on_close.run(());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    view! {
        <Modal
            title=if reconnect { "Reconnect database" } else { "Connect a database" }
            subtitle="Discover the structure behind your data."
            on_close=on_close
            busy=busy
        >
            <form on:submit=submit>
                <fieldset class="mb-[25px] grid grid-cols-3 gap-2">
                    <legend class="form-label mb-3">"Database engine"</legend>
                    {DATABASE_KINDS.iter().map(|(value, label)| {
                        let value: &'static str = value;
                        view! {
                            <label class="flex cursor-pointer items-center gap-[7px] rounded-md border border-line-soft bg-surface px-[9px] py-3 text-[10px] text-text transition-colors has-checked:border-soft">
                                <input
                                    type="radio"
                                    class="m-0 size-[11px] accent-accent"
                                    name="engine"
                                    value=value
                                    prop:checked=move || kind.get() == value
                                    on:change=move |_| {
                                        kind.set(value.into());
                                        dsn.set(String::new());
                                    }
                                />
                                <Icon name="database" size=16 class="w-[13px]" />
                                {*label}
                            </label>
                        }
                    }).collect_view()}
                </fieldset>
                <label class="form-label" for="connection-name">"Connection name " <span class="text-accent">"*"</span></label>
                <input id="connection-name" class="field mb-[21px]" name="name" bind:value=name placeholder="e.g. Production database" maxlength="80" required />
                <label class="form-label" for="connection-string">
                    {move || if sqlite() { "Database file " } else { "Connection string " }}
                    <span class="text-accent">"*"</span>
                </label>
                <div class="field mb-2 flex items-center gap-1.5 py-1.5 pr-[7px] pl-3">
                    <input
                        id="connection-string"
                        class="w-full min-w-0 border-0 bg-transparent py-[5px] font-mono text-[11px]"
                        name="connection"
                        type=move || if sqlite() || reveal.get() { "text" } else { "password" }
                        autocomplete="off"
                        bind:value=dsn
                        placeholder=move || placeholder(&kind.get())
                        required
                        maxlength="8192"
                        spellcheck="false"
                    />
                    {move || if sqlite() {
                        view! {
                            <button type="button" class="icon-btn" aria-label="Choose database file" on:click=browse>
                                <Icon name="folder-open" size=17 />
                            </button>
                        }.into_any()
                    } else {
                        view! {
                            <button
                                type="button"
                                class="icon-btn"
                                aria-label=move || if reveal.get() { "Hide connection string" } else { "Show connection string" }
                                on:click=move |_| reveal.update(|r| *r = !*r)
                            >
                                {move || if reveal.get() { view! { <Icon name="eye-off" size=17 /> } } else { view! { <Icon name="eye" size=17 /> } }}
                            </button>
                        }.into_any()
                    }}
                </div>
                <p class="form-hint mb-[22px] font-mono text-[8px] [overflow-wrap:anywhere]">{move || placeholder(&kind.get())}</p>
                <div class="flex gap-2.5 rounded-md bg-surface p-3.5 text-soft">
                    <Icon name="shield-check" size=18 />
                    <p class="text-[10px] leading-[1.8]">
                        "Only schema metadata is inspected. Credentials stay in memory for this session. Saved maps remain available offline."
                    </p>
                </div>
                <FormError error=error />
                <div class="modal-footer">
                    <button type="button" class="btn" disabled=move || busy.get() on:click=move |_| on_close.run(())>"Cancel"</button>
                    <button type="submit" class="btn btn-primary" disabled=move || busy.get()>
                        {move || if busy.get() { "Inspecting schema…" } else { "Connect & map" }}
                        <Icon name="arrow-right" size=16 />
                    </button>
                </div>
            </form>
        </Modal>
    }
}

#[component]
pub fn ImportDialog(
    project_id: String,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_save: Callback<Project>,
) -> impl IntoView {
    let file = RwSignal::new_local(None::<web_sys::File>);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let choose = move |ev: leptos::ev::Event| {
        error.set(String::new());
        file.set(None);
        let input: web_sys::HtmlInputElement = event_target(&ev);
        let Some(selected) = input.files().and_then(|list| list.get(0)) else {
            return;
        };
        if selected.size() > 20.0 * 1024.0 * 1024.0 {
            error.set("Choose a JSON file smaller than 20 MB.".into());
            return;
        }
        file.set(Some(selected));
    };
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(selected) = file.get_untracked() else {
            error.set("Choose an OpenAPI JSON file first.".into());
            return;
        };
        let project_id = project_id.clone();
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let text = wasm_bindgen_futures::JsFuture::from(selected.text())
                .await
                .map_err(api::js_error)
                .and_then(|v| {
                    v.as_string()
                        .ok_or_else(|| "Could not read the file.".to_string())
                });
            let result = match text {
                Ok(document) => api::import_openapi(&project_id, &document).await,
                Err(e) => Err(e),
            };
            match result {
                Ok(p) => {
                    on_save.run(p);
                    on_close.run(());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    let file_label = move || file.with(|f| f.as_ref().map(|f| f.name()));
    view! {
        <Modal
            title="Your API, connected."
            subtitle="Import a definition to explore endpoints, models, and references."
            on_close=on_close
            busy=busy
        >
            <form on:submit=submit>
                <label
                    class="relative mb-4 flex cursor-pointer flex-col items-center rounded-[9px] border border-dashed border-soft bg-surface px-[15px] pt-[35px] pb-[25px] transition-colors hover:bg-surface focus-within:outline-2 focus-within:outline-offset-3 focus-within:outline-accent"
                    for="openapi-file"
                >
                    <div class="tile"><Icon name="file-json" size=28 /></div>
                    <strong class="mt-[18px] max-w-full text-sm font-medium [overflow-wrap:anywhere]">
                        {move || file_label().unwrap_or_else(|| "Choose an OpenAPI file".into())}
                    </strong>
                    <span class="mt-2.5 mb-5 text-[9px] text-soft">
                        {move || match file.with(|f| f.as_ref().map(|f| f.size())) {
                            Some(size) => format!("{:.1} KB · Ready to import", size / 1024.0),
                            None => "OpenAPI 3.x or Swagger 2.0 · JSON · Up to 20 MB".into(),
                        }}
                    </span>
                    <span class="btn text-[10px]">
                        <Icon name="upload" size=15 />
                        {move || if file_label().is_some() { "Choose another file" } else { "Browse files" }}
                    </span>
                    <input id="openapi-file" name="file" class="sr-only" type="file" accept=".json,application/json" on:change=choose />
                </label>
                <p class="form-hint">
                    "Local schema references become connections. External references are reported without fetching remote files."
                </p>
                <FormError error=error />
                <div class="modal-footer">
                    <button type="button" class="btn" on:click=move |_| on_close.run(()) disabled=move || busy.get()>"Cancel"</button>
                    <button class="btn btn-primary" type="submit" disabled=move || busy.get()>
                        {move || if busy.get() { "Mapping definition…" } else { "Import definition" }}
                        <Icon name="arrow-right" size=16 />
                    </button>
                </div>
            </form>
        </Modal>
    }
}

#[component]
pub fn ConfirmDialog(
    #[prop(into)] title: String,
    #[prop(into)] message: String,
    #[prop(into)] on_close: Callback<()>,
    on_confirm: Action,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let confirm = move |_| {
        let on_confirm = on_confirm.clone();
        spawn_local(async move {
            busy.set(true);
            match on_confirm().await {
                Ok(()) => on_close.run(()),
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    view! {
        <Modal title=title on_close=on_close busy=busy>
            <p class="text-[13px] leading-[1.9] text-ink">{message}</p>
            <FormError error=error />
            <div class="modal-footer">
                <button type="button" class="btn" on:click=move |_| on_close.run(()) disabled=move || busy.get()>"Cancel"</button>
                <button type="button" class="btn btn-danger" on:click=confirm disabled=move || busy.get()>
                    {move || if busy.get() { "Removing…" } else { "Delete" }}
                </button>
            </div>
        </Modal>
    }
}

#[component]
pub fn ApiDialog(
    project_id: String,
    source: Arc<Source>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_save: Callback<Project>,
) -> impl IntoView {
    let base_url = RwSignal::new(source.api_base_url.clone().unwrap_or_default());
    let headers = RwSignal::new("{}".to_string());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let source_id = source.id.clone();
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let (project_id, source_id) = (project_id.clone(), source_id.clone());
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            let parsed: Result<serde_json::Value, String> =
                serde_json::from_str(&headers.get_untracked())
                    .map_err(|e| e.to_string())
                    .and_then(|v: serde_json::Value| match v.as_object() {
                        Some(map) if map.values().all(|v| v.is_string()) => Ok(v),
                        _ => Err("Headers must be a JSON object with string values.".into()),
                    });
            let result = match parsed {
                Ok(headers) => {
                    api::configure_api(&project_id, &source_id, &base_url.get_untracked(), headers)
                        .await
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(p) => {
                    on_save.run(p);
                    on_close.run(());
                }
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    let _ = database_name;
    view! {
        <Modal title="Connect this API" subtitle="Choose the server your agent can send requests to." on_close=on_close busy=busy>
            <form on:submit=submit>
                <label class="form-label" for="api-url">"Base URL"</label>
                <input id="api-url" class="field mb-[21px]" type="url" required bind:value=base_url placeholder="http://localhost:3000/v1" />
                <p class="form-hint">"Operation paths append to this URL. Redirects are not followed."</p>
                <label class="form-label mt-4" for="api-headers">
                    "Headers " <small class="float-right text-[10px] font-normal text-muted">"JSON object · held in memory"</small>
                </label>
                <textarea id="api-headers" class="field mb-[21px] max-h-60 min-h-[90px] resize-y" rows="4" bind:value=headers spellcheck="false" autocomplete="off" placeholder=r#"{"Authorization":"Bearer …"}"#></textarea>
                <p class="form-hint">"Headers are cleared when the app closes. Each agent request needs your approval."</p>
                <FormError error=error />
                <div class="modal-footer">
                    <button class="btn btn-primary" type="submit" disabled=move || busy.get()>
                        {move || if busy.get() { "Saving…" } else { "Save API connection" }}
                    </button>
                </div>
            </form>
        </Modal>
    }
}

/// Read a form control from an event target.
pub fn event_target<T: JsCast>(ev: &leptos::ev::Event) -> T {
    ev.target().unwrap().unchecked_into::<T>()
}
