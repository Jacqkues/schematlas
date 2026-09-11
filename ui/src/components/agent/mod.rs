//! Local coding agent panel: ACP session controls, transcript, reviews and discovery.
use crate::api;
use crate::components::icons::Icon;
use crate::markdown::render_markdown;
use crate::types::{AgentSnapshot, InstalledAgent, Review};
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::time::Duration;

const CHAT_WIDTH_KEY: &str = "atlas.chat.width";
const PRESET_KEY: &str = "atlas:agent-preset";
const MINIMUM: f64 = 320.0;

/// A side panel with a keyboard- and pointer-resizable left edge (WAI-ARIA window splitter).
#[component]
pub fn ResizablePanel(children: Children) -> impl IntoView {
    let saved = api::local_storage_get(CHAT_WIDTH_KEY)
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|w| *w >= MINIMUM);
    let width = RwSignal::new(saved.unwrap_or(420.0));
    let viewport = RwSignal::new(
        web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
            .unwrap_or(1440.0),
    );
    let dragging = RwSignal::new(false);
    let origin = StoredValue::new(None::<(f64, f64)>);
    let maximum = Memo::new(move |_| {
        let v = viewport.get();
        MINIMUM.max(v - if v > 1100.0 { 640.0 } else { 96.0 })
    });
    let actual = Memo::new(move |_| width.get().clamp(MINIMUM, maximum.get()));
    let save = move || api::local_storage_set(CHAT_WIDTH_KEY, &actual.get_untracked().to_string());
    let _resize = window_event_listener(leptos::ev::resize, move |_| {
        if let Some(w) = web_sys::window()
            .and_then(|w| w.inner_width().ok())
            .and_then(|v| v.as_f64())
        {
            viewport.set(w);
        }
    });
    let start = move |ev: leptos::ev::PointerEvent| {
        if ev.button() != 0 {
            return;
        }
        origin.set_value(Some((ev.client_x() as f64, actual.get_untracked())));
        dragging.set(true);
        if let Some(target) = ev
            .current_target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
        {
            let _ = target.set_pointer_capture(ev.pointer_id());
        }
        ev.prevent_default();
    };
    let moving = move |ev: leptos::ev::PointerEvent| {
        if let Some((x, w)) = origin.get_value() {
            width.set((w + x - ev.client_x() as f64).clamp(MINIMUM, maximum.get_untracked()));
        }
    };
    let stop = move |_ev: leptos::ev::PointerEvent| {
        if origin.get_value().is_some() {
            origin.set_value(None);
            dragging.set(false);
            save();
        }
    };
    let key = move |ev: leptos::ev::KeyboardEvent| {
        let step = if ev.shift_key() { 80.0 } else { 20.0 };
        let (current, max) = (actual.get_untracked(), maximum.get_untracked());
        match ev.key().as_str() {
            "ArrowLeft" => width.set((current + step).min(max)),
            "ArrowRight" => width.set((current - step).max(MINIMUM)),
            "Home" => width.set(MINIMUM),
            "End" => width.set(max),
            _ => return,
        }
        ev.prevent_default();
        save();
    };
    view! {
        <div
            class="relative flex min-h-0 shrink-0 animate-fade-in [&>.agent-panel]:w-(--chat-width) [&>.agent-panel]:shrink-0 max-[1100px]:absolute max-[1100px]:inset-y-0 max-[1100px]:right-0 max-[1100px]:z-30 max-[1100px]:shadow-[-10px_0_30px_#0005]"
            class=("select-none", move || dragging.get())
            style:width=move || format!("{}px", actual.get())
            style=("--chat-width", move || format!("{}px", actual.get()))
        >
            <div
                class="absolute inset-y-0 -left-[5px] z-40 w-2.5 cursor-col-resize touch-none after:absolute after:left-1 after:h-full after:w-0.5 after:bg-transparent after:transition-colors hover:after:bg-accent-muted focus-visible:after:bg-accent-muted"
                class=("after:bg-accent-muted", move || dragging.get())
                role="separator"
                tabindex="0"
                aria-label="Resize agent chat"
                aria-orientation="vertical"
                aria-valuemin=MINIMUM
                aria-valuemax=move || maximum.get()
                aria-valuenow=move || actual.get().round()
                on:pointerdown=start
                on:pointermove=moving
                on:pointerup=stop
                on:pointercancel=stop
                on:keydown=key
                on:dblclick=move |_| {
                    width.set(420.0);
                    save();
                }
            ></div>
            {children()}
        </div>
    }
}

fn message_text_class() -> &'static str {
    "my-[7px] text-xs leading-[1.8] whitespace-pre-wrap text-ink [overflow-wrap:anywhere]"
}

#[component]
pub fn AgentPanel(
    project_id: String,
    project_name: String,
    #[prop(into)] on_close: Callback<()>,
) -> impl IntoView {
    let desktop = api::desktop();
    let snapshot = RwSignal::new(None::<AgentSnapshot>);
    let executable = RwSignal::new(String::new());
    let args = RwSignal::new("[]".to_string());
    let cwd = RwSignal::new(String::new());
    let loading_directory = RwSignal::new(true);
    let prompt = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let transcript = NodeRef::<leptos::html::Div>::new();
    let unread = RwSignal::new(false);
    let status = Memo::new(move |_| {
        snapshot.with(|s| s.as_ref().map(|s| s.status.clone()).unwrap_or_default())
    });
    let connected = Memo::new(move |_| {
        snapshot.with(|s| {
            s.as_ref()
                .is_some_and(|s| !matches!(s.status.as_str(), "disconnected" | "error"))
        })
    });
    let ready = move || status.get() == "ready";
    let running = move || matches!(status.get().as_str(), "running" | "cancelling");

    let near_bottom = move || {
        transcript
            .get_untracked()
            .map(|el| (el.scroll_height() - el.scroll_top() - el.client_height()) < 100)
            .unwrap_or(true)
    };
    let show_latest = move |smooth: bool| {
        request_animation_frame(move || {
            if let Some(el) = transcript.get_untracked() {
                let options = web_sys::ScrollToOptions::new();
                options.set_top(el.scroll_height() as f64);
                options.set_behavior(if smooth {
                    web_sys::ScrollBehavior::Smooth
                } else {
                    web_sys::ScrollBehavior::Instant
                });
                el.scroll_to_with_scroll_to_options(&options);
            }
            unread.set(false);
        });
    };
    let remember_preset = move || {
        let value = serde_json::json!({ "executable": executable.get_untracked(), "args": args.get_untracked() });
        api::local_storage_set(PRESET_KEY, &value.to_string());
    };
    let restore_preset = move || {
        if let Some(saved) = api::local_storage_get(PRESET_KEY)
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        {
            if let (Some(exe), Some(a)) = (
                saved.get("executable").and_then(|v| v.as_str()),
                saved.get("args").and_then(|v| v.as_str()),
            ) {
                executable.set(exe.to_string());
                args.set(a.to_string());
            }
        }
    };

    if desktop {
        if executable.get_untracked().is_empty() {
            restore_preset();
        }
        let listener_project = project_id.clone();
        let listener = api::listen::<AgentSnapshot>("agent:update", move |event| {
            if event.project_id != listener_project {
                return;
            }
            let bottom = near_bottom();
            snapshot.set(Some(event));
            if bottom {
                show_latest(false);
            } else {
                unread.set(true);
            }
        });
        let _keep = StoredValue::new_local(listener);
        let id = project_id.clone();
        spawn_local(async move {
            match api::agent_working_directory(&id).await {
                Ok(directory) => {
                    if cwd.get_untracked().is_empty() {
                        cwd.set(directory);
                    }
                }
                Err(e) => error.set(e),
            }
            loading_directory.set(false);
        });
        let id = project_id.clone();
        spawn_local(async move {
            match api::agent_status(&id).await {
                Ok(value) => {
                    if snapshot.get_untracked().is_none() {
                        snapshot.set(value);
                        show_latest(false);
                    }
                }
                Err(e) => error.set(e),
            }
        });
    }

    let perform = {
        let project_id = project_id.clone();
        move |task: AgentTask| {
            let project_id = project_id.clone();
            spawn_local(async move {
                error.set(String::new());
                let result = match task {
                    AgentTask::Connect => {
                        busy.set(true);
                        let parsed: Result<Vec<String>, String> =
                            serde_json::from_str::<serde_json::Value>(&args.get_untracked())
                                .ok()
                                .and_then(|v| v.as_array().cloned())
                                .and_then(|items| {
                                    items
                                        .iter()
                                        .map(|i| i.as_str().map(str::to_string))
                                        .collect::<Option<Vec<_>>>()
                                })
                                .ok_or_else(|| {
                                    "Arguments must be a JSON array of strings.".to_string()
                                });
                        let result = match parsed {
                            Ok(list) => {
                                api::agent_connect(
                                    &project_id,
                                    &executable.get_untracked(),
                                    &list,
                                    &cwd.get_untracked(),
                                )
                                .await
                            }
                            Err(e) => Err(e),
                        };
                        if result.is_ok() {
                            remember_preset();
                        }
                        busy.set(false);
                        result
                    }
                    AgentTask::Prompt(text) => api::agent_prompt(&project_id, &text).await,
                    AgentTask::Cancel => api::agent_cancel(&project_id).await,
                    AgentTask::Disconnect => api::agent_disconnect(&project_id).await,
                    AgentTask::Decide(review_id, option) => {
                        api::agent_decide(&project_id, &review_id, option.as_deref()).await
                    }
                    AgentTask::Authenticate(method) => {
                        busy.set(true);
                        let r = api::agent_authenticate(&project_id, &method).await;
                        busy.set(false);
                        r
                    }
                    AgentTask::Browse(directory) => {
                        let title = if directory {
                            "Agent working directory"
                        } else {
                            "ACP executable"
                        };
                        let default = directory.then(|| cwd.get_untracked());
                        match api::open_dialog(title, directory, default.as_deref()).await {
                            Ok(Some(path)) => {
                                if directory {
                                    cwd.set(path);
                                } else {
                                    executable.set(path);
                                }
                                Ok(())
                            }
                            Ok(None) => Ok(()),
                            Err(e) => Err(e),
                        }
                    }
                };
                if let Err(e) = result {
                    error.set(e);
                }
            });
        }
    };
    let perform = StoredValue::new_local(perform);
    let run = move |task: AgentTask| perform.with_value(|p| p(task));
    let send = move || {
        let text = prompt.get_untracked();
        if text.trim().is_empty() {
            return;
        }
        prompt.set(String::new());
        run(AgentTask::Prompt(text));
    };
    let composer_keydown = move |ev: leptos::ev::KeyboardEvent| {
        // Enter sends; any modifier keeps inserting a newline. Ignore Enter that confirms an IME composition.
        if ev.key() != "Enter"
            || ev.shift_key()
            || ev.alt_key()
            || ev.ctrl_key()
            || ev.meta_key()
            || ev.is_composing()
        {
            return;
        }
        ev.prevent_default();
        if ready() {
            send();
        }
    };
    let on_select_preset = Callback::new(move |installed: InstalledAgent| {
        executable.set(installed.executable);
        args.set(serde_json::to_string(&installed.args).unwrap_or_else(|_| "[]".into()));
        remember_preset();
    });
    let label_class = "mt-5 mb-[7px] block text-[11px] text-soft";
    let input_class = "w-full rounded-[7px] border border-line-soft bg-field p-2.5 font-mono text-[11px] text-ink";
    let note_class = "py-1 text-[11px] leading-relaxed text-muted";
    let agent_name =
        move || snapshot.with(|s| s.as_ref().map(|s| s.agent_name.clone()).unwrap_or_default());

    view! {
        <aside class="agent-panel flex min-h-0 flex-col border-l border-line-soft bg-surface text-ink" aria-label="Local coding agent">
            <header class="flex items-center justify-between px-5 pt-6 pb-[15px]">
                <div>
                    <span class="eyebrow">"ACP SESSION"</span>
                    <h2 class="mt-[9px] flex items-center gap-[9px] text-lg font-bold"><Icon name="terminal" size=18 /> "Local agent"</h2>
                </div>
                <button type="button" class="icon-btn" aria-label="Close agent panel" on:click=move |_| on_close.run(())><Icon name="x" size=18 /></button>
            </header>
            <div class="flex items-center gap-[7px] border-y border-line px-5 py-3 text-[11px]">
                <span class="status-dot"></span>
                {project_name}
                <small class="ml-auto text-[10px] text-soft capitalize">{move || { let s = status.get(); if s.is_empty() { "Not connected".to_string() } else { s } }}</small>
            </div>
            {move || if !desktop {
                view! { <p class="p-5 text-[11px] leading-relaxed text-muted">"Agent sessions run in the desktop app."</p> }.into_any()
            } else if !connected.get() {
                view! {
                    <form class="overflow-auto p-[22px]" on:submit=move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); run(AgentTask::Connect); }>
                        <AgentDiscovery selected=executable on_select=on_select_preset />
                        <h3 class="text-base font-semibold">"Your coding agent, in context."</h3>
                        <p class="text-xs leading-relaxed text-soft">"Connect an installed ACP agent or adapter. This session can inspect this project’s databases and APIs."</p>
                        <label class=label_class for="agent-executable">"ACP executable"</label>
                        <div class="flex gap-[5px]">
                            <input id="agent-executable" spellcheck="false" {leptos::tachys::html::attribute::custom::custom_attribute("autocorrect", "off")} autocapitalize="off" class=format!("{input_class} min-w-0") placeholder="/absolute/path/to/agent" bind:value=executable required />
                            <button type="button" class="icon-btn" aria-label="Choose agent executable" on:click=move |_| run(AgentTask::Browse(false))><Icon name="folder-open" size=16 /></button>
                        </div>
                        <label class=label_class for="agent-args">"Arguments " <small class="ml-[5px] text-muted">"JSON array"</small></label>
                        <input id="agent-args" spellcheck="false" {leptos::tachys::html::attribute::custom::custom_attribute("autocorrect", "off")} autocapitalize="off" class=input_class bind:value=args placeholder=r#"["--acp"]"# required />
                        <label class=label_class for="agent-cwd">"Working directory"</label>
                        <div class="flex gap-[5px]">
                            <input id="agent-cwd" spellcheck="false" {leptos::tachys::html::attribute::custom::custom_attribute("autocorrect", "off")} autocapitalize="off" class=format!("{input_class} min-w-0") name="workingDirectory" aria-describedby="agent-directory-help" placeholder="/absolute/path/to/project" bind:value=cwd required />
                            <button type="button" class="icon-btn" aria-label="Choose working directory" on:click=move |_| run(AgentTask::Browse(true))><Icon name="folder-open" size=16 /></button>
                        </div>
                        <p class=note_class id="agent-directory-help">
                            {move || if loading_directory.get() { "Preparing your project folder…" } else { "Your project folder is selected automatically. Choose an existing repository to use it instead; your choice is remembered, as is the last executable you connected." }}
                        </p>
                        <button type="submit" class="btn btn-primary mt-[22px]" disabled=move || busy.get() || (loading_directory.get() && cwd.with(|c| c.is_empty()))>
                            <Icon name="plug-zap" size=15 />
                            {move || if busy.get() { "Connecting…" } else { "Connect agent" }}
                        </button>
                        <p class=note_class>"Use an ACP-compatible adapter, not a regular interactive CLI. SQL and HTTP requests ask for your approval here. Agent file operations follow its own permission settings."</p>
                    </form>
                }.into_any()
            } else {
                view! {
                    <div class="flex items-center justify-between px-5 py-[13px] text-xs">
                        <strong>{agent_name}</strong>
                        <button type="button" class="btn px-2.5 py-1.5 text-[10px]" on:click=move |_| run(AgentTask::Disconnect)>"Disconnect"</button>
                    </div>
                    <Show when=move || status.get() == "authentication">
                        <div class="p-[22px]">
                            <p class="text-xs leading-relaxed text-soft">"Authenticate with your agent to start a session."</p>
                            {move || snapshot.with(|s| s.as_ref().map(|s| s.auth_methods.clone()).unwrap_or_default()).into_iter().map(|method| {
                                let id = method.id.clone();
                                view! { <button type="button" class="btn" disabled=move || busy.get() on:click=move |_| run(AgentTask::Authenticate(id.clone()))>{method.name.clone()}</button> }
                            }).collect_view()}
                        </div>
                    </Show>
                    {move || snapshot.get().map(|s| view! { <AgentActivity snapshot=s /> })}
                    <div
                        node_ref=transcript
                        class="min-h-[100px] flex-1 overflow-auto overscroll-contain px-[18px] pt-2 pb-5"
                        role="log"
                        aria-label="Agent conversation"
                        aria-live="polite"
                        on:scroll=move |_| { if near_bottom() { unread.set(false); } }
                    >
                        <Show when=move || snapshot.with(|s| s.as_ref().is_none_or(|s| s.messages.is_empty()))>
                            <div class="px-2 py-[45px] text-soft">
                                <Icon name="terminal" size=26 />
                                <h3 class="text-base font-semibold">"Ask about your architecture."</h3>
                                <p class="text-xs leading-[1.9] text-muted">"“Group my tables by domain and arrange the map.”" <br /> "“Find the API endpoint for creating an order.”"</p>
                            </div>
                        </Show>
                        <For
                            each=move || snapshot.with(|s| s.as_ref().map(|s| s.messages.clone()).unwrap_or_default())
                            key=|m| (m.id.clone(), m.role.clone())
                            children=move |message| {
                                let user = message.role == "user";
                                let tool = message.role == "tool";
                                let who = match message.role.as_str() { "user" => "You".to_string(), "tool" => "Tool".to_string(), _ => agent_name() };
                                let assistant = message.role == "assistant";
                                let message_id = message.id.clone();
                                let current = Memo::new(move |_| snapshot.with(|s| {
                                    s.as_ref().and_then(|s| s.messages.iter().find(|m| m.id == message_id)).cloned().unwrap_or_default()
                                }));
                                let status_suffix = move || current.with(|m| m.status.as_ref().map(|s| format!(" · {s}")).unwrap_or_default());
                                let text = Signal::derive(move || current.with(|m| m.text.clone()));
                                let first_line = move || text.with(|text| text.lines().next().unwrap_or_default().to_string());
                                view! {
                                    <article
                                        class="my-[18px] [contain-intrinsic-size:auto_100px] [content-visibility:auto]"
                                        class=("rounded-[10px]", user) class=("border", user) class=("border-line-soft", user) class=("bg-surface", user) class=("px-3.5", user) class=("py-3", user)
                                    >
                                        <span class="text-[10px] font-semibold text-muted">{who}{status_suffix}</span>
                                        {if tool {
                                            view! {
                                                <details>
                                                    <summary class="mt-1.5 cursor-pointer truncate font-mono text-[11px] leading-relaxed text-soft">{first_line}</summary>
                                                    <p class=format!("{} max-h-[200px] overflow-auto font-mono text-[11px] text-soft", message_text_class())>{move || text.get()}</p>
                                                </details>
                                            }.into_any()
                                        } else if assistant {
                                            view! { <AgentMarkdown text=text on_render=Callback::new(move |_: ()| { if !unread.get_untracked() { show_latest(false); } }) /> }.into_any()
                                        } else {
                                            view! { <p class=message_text_class()>{move || text.get()}</p> }.into_any()
                                        }}
                                    </article>
                                }
                            }
                        />
                    </div>
                    <Show when=move || unread.get()>
                        <button type="button" class="btn mx-4 mb-3 animate-fade-in self-center text-[11px]" on:click=move |_| show_latest(true)>"Latest activity ↓"</button>
                    </Show>
                    <Show when=move || snapshot.with(|s| s.as_ref().is_some_and(|s| !s.reviews.is_empty()))>
                        <div class="max-h-[40%] shrink-0 overflow-auto border-t border-line px-4 pb-3" aria-label="Pending approvals">
                            <For each=move || snapshot.with(|s| s.as_ref().map(|s| s.reviews.clone()).unwrap_or_default()) key=|r| r.id.clone() children=move |review| {
                                let review_id = review.id.clone();
                                view! { <ReviewCard review=review on_decide=Callback::new(move |option: Option<String>| run(AgentTask::Decide(review_id.clone(), option))) /> }
                            } />
                        </div>
                    </Show>
                    <form class="mx-4 mb-4 rounded-[11px] border border-line-strong bg-surface-3 p-3" on:submit=move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); send(); }>
                        <label class="sr-only" for="agent-prompt">"Message your agent"</label>
                        <textarea
                            id="agent-prompt"
                            class="max-h-[200px] w-full resize-y border-0 bg-transparent text-xs leading-relaxed text-ink outline-none focus-visible:outline-none"
                            bind:value=prompt
                            rows="3"
                            maxlength="65536"
                            placeholder="Ask about this project…"
                            aria-describedby="agent-composer-help"
                            on:keydown=composer_keydown
                            disabled=move || !ready()
                        ></textarea>
                        <div class="flex items-center justify-between">
                            <span id="agent-composer-help" class="text-[10px] text-soft">"Enter sends · Shift+Enter for a new line"</span>
                            {move || if running() {
                                view! { <button type="button" class="btn" on:click=move |_| run(AgentTask::Cancel)><Icon name="square" size=13 /> "Stop"</button> }.into_any()
                            } else {
                                view! {
                                    <button type="submit" class="btn btn-primary" aria-label="Send message" disabled=move || !ready() || prompt.with(|p| p.trim().is_empty())>
                                        <Icon name="arrow-up" size=16 />
                                    </button>
                                }.into_any()
                            }}
                        </div>
                    </form>
                }.into_any()
            }}
            <Show when=move || !error.get().is_empty() || snapshot.with(|s| s.as_ref().is_some_and(|s| s.error.is_some()))>
                <p class="form-error mx-[18px] mb-[18px] max-h-[140px] overflow-auto text-xs" role="alert">
                    {move || { let e = error.get(); if e.is_empty() { snapshot.with(|s| s.as_ref().and_then(|s| s.error.clone()).unwrap_or_default()) } else { e } }}
                </p>
            </Show>
        </aside>
    }
}

#[derive(Clone, Debug)]
enum AgentTask {
    Connect,
    Prompt(String),
    Cancel,
    Disconnect,
    Decide(String, Option<String>),
    Authenticate(String),
    Browse(bool),
}

#[component]
fn AgentDiscovery(
    #[prop(into)] selected: Signal<String>,
    #[prop(into)] on_select: Callback<InstalledAgent>,
) -> impl IntoView {
    let agents = RwSignal::new(Vec::<InstalledAgent>::new());
    let busy = RwSignal::new(true);
    let error = RwSignal::new(String::new());
    let scan = move || {
        spawn_local(async move {
            busy.set(true);
            error.set(String::new());
            match api::discover_agents().await {
                Ok(list) => agents.set(list),
                Err(e) => error.set(e),
            }
            busy.set(false);
        });
    };
    scan();
    let note = "my-2 text-[10px] leading-relaxed";
    view! {
        <section class="mb-6 rounded-[9px] border border-line-strong bg-surface p-3" aria-label="Installed agents">
            <div class="flex items-center justify-between">
                <span class="eyebrow">"ON THIS COMPUTER"</span>
                <button type="button" class="icon-btn" aria-label="Rescan installed agents" disabled=move || busy.get() on:click=move |_| scan()><Icon name="refresh-cw" size=13 /></button>
            </div>
            {move || if busy.get() {
                view! { <p class=note>"Looking for installed agents…"</p> }.into_any()
            } else {
                let list = agents.get();
                let needs_adapter = list.iter().any(|a| !a.acp_ready);
                view! {
                    {if list.is_empty() {
                        view! { <p class=note>"No known agent executables found. You can choose a custom executable below."</p> }.into_any()
                    } else {
                        list.into_iter().map(|installed| {
                            let exe = installed.executable.clone();
                            let active = Memo::new(move |_| selected.get() == exe);
                            let ready = installed.acp_ready;
                            let chosen = installed.clone();
                            view! {
                                <button
                                    type="button"
                                    class="flex w-full items-center gap-2.5 border-t border-line px-2 py-3 text-left text-text transition-colors not-disabled:hover:bg-accent-soft not-disabled:hover:text-accent-text disabled:opacity-65 aria-pressed:text-accent-text"
                                    disabled=!ready
                                    aria-pressed=move || active.get().to_string()
                                    title=installed.executable.clone()
                                    on:click=move |_| on_select.run(chosen.clone())
                                >
                                    <Icon name="terminal" size=16 />
                                    <span class="flex-1">
                                        <strong class="block text-xs">{installed.name.clone()}</strong>
                                        <small class="mt-[5px] block text-[10px] text-accent-muted">
                                            {move || if active.get() { "Selected" } else if ready { "ACP preset available" } else { "Installed · ACP adapter needed" }}
                                        </small>
                                    </span>
                                    <Show when=move || active.get()><Icon name="check" size=14 /></Show>
                                </button>
                            }
                        }).collect_view().into_any()
                    }}
                    <Show when=move || needs_adapter>
                        <p class=note>"Standalone Claude and Codex CLIs need an ACP adapter. Detection never starts an agent."</p>
                    </Show>
                }.into_any()
            }}
            <Show when=move || !error.get().is_empty()><p class="form-error" role="alert">{move || error.get()}</p></Show>
        </section>
    }
}

fn duration(seconds: f64) -> String {
    let seconds = seconds.max(0.0).floor() as i64;
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}

#[component]
fn AgentActivity(snapshot: AgentSnapshot) -> impl IntoView {
    let now = RwSignal::new(api::now_ms());
    let timer =
        set_interval_with_handle(move || now.set(api::now_ms()), Duration::from_secs(1)).ok();
    on_cleanup(move || {
        if let Some(handle) = timer {
            handle.clear();
        }
    });
    let running = matches!(snapshot.status.as_str(), "running" | "cancelling");
    let waiting = !snapshot.reviews.is_empty();
    if !running && !waiting {
        return ().into_any();
    }
    let last_activity = snapshot.last_activity_at;
    let started = snapshot.turn_started_at;
    let silence = move || ((now.get() - last_activity) / 1000.0).max(0.0);
    let elapsed = move || ((now.get() - started.unwrap_or_else(|| now.get())) / 1000.0).max(0.0);
    let active_tools = snapshot
        .messages
        .iter()
        .rposition(|m| m.role == "user")
        .map(|start| {
            snapshot.messages[start..]
                .iter()
                .filter(|m| {
                    m.role == "tool"
                        && !matches!(m.status.as_deref(), Some("completed") | Some("failed"))
                })
                .count()
        })
        .unwrap_or(0);
    let status = snapshot.status.clone();
    let activity = snapshot.activity.clone();
    let label = move || {
        if waiting {
            "Waiting for your approval".to_string()
        } else if status == "cancelling" {
            "Stopping…".into()
        } else if silence() >= 30.0 {
            "Waiting for agent activity".into()
        } else {
            match activity.as_str() {
                "thinking" => "Thinking…".into(),
                "planning" => "Planning…".into(),
                "compacting" => "Compacting context…".into(),
                "responding" => "Writing a response…".into(),
                _ if active_tools > 0 => format!(
                    "Working on {active_tools} tool {}…",
                    if active_tools == 1 { "call" } else { "calls" }
                ),
                _ => "Waiting for agent…".into(),
            }
        }
    };
    let hint = move || {
        if waiting {
            "Review the request below to continue.".to_string()
        } else if silence() >= 30.0 {
            format!(
                "No update for {}. You can stop this run if needed.",
                duration(silence())
            )
        } else {
            "Live activity from your agent. Canvas edits appear as they are saved.".into()
        }
    };
    view! {
        <div class="mx-4 mb-3 animate-fade-in rounded-lg border border-line bg-surface px-3 py-[11px]">
            <div class="flex items-center gap-2">
                <span class="size-1.5 shrink-0 rounded-full" class=("bg-warning", waiting) class=("bg-accent", !waiting)></span>
                <strong class="text-[11px] font-medium text-ink" role="status">{label}</strong>
                <time class="ml-auto font-mono text-[10px] whitespace-nowrap text-muted tabular-nums">{move || duration(elapsed())}</time>
            </div>
            <p class="mt-[7px] text-[10px] leading-relaxed text-muted">{hint}</p>
        </div>
    }
    .into_any()
}

#[component]
fn ReviewCard(review: Review, #[prop(into)] on_decide: Callback<Option<String>>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let decide = move |option: Option<String>| {
        busy.set(true);
        on_decide.run(option);
    };
    let details = serde_json::to_string_pretty(&review.details).unwrap_or_default();
    view! {
        <section class="my-[18px] rounded-[9px] border border-accent-line bg-accent-soft p-3.5" aria-label="Permission request">
            <span class="eyebrow text-muted">{format!("REVIEW REQUIRED · {}", review.kind)}</span>
            <h3 class="text-[13px] [overflow-wrap:anywhere]">{review.title.clone()}</h3>
            <pre class="max-h-[250px] overflow-auto rounded-[5px] bg-accent-soft p-[9px] font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-ink [overflow-wrap:anywhere]">{details}</pre>
            <div class="flex flex-wrap gap-1.5">
                {review.options.iter().map(|option| {
                    let id = option.option_id.clone();
                    let primary = option.kind == "allow_once";
                    view! {
                        <button type="button" class="btn px-2.5 py-[7px] text-[10px]" class=("btn-primary", primary) disabled=move || busy.get() on:click=move |_| decide(Some(id.clone()))>{option.name.clone()}</button>
                    }
                }).collect_view()}
                <button type="button" class="btn px-2.5 py-[7px] text-[10px]" disabled=move || busy.get() on:click=move |_| decide(None)>"Cancel"</button>
            </div>
        </section>
    }
}

/// Streams sanitized Markdown at most ten renders per second, even when tokens arrive faster.
#[component]
fn AgentMarkdown(
    #[prop(into)] text: Signal<String>,
    #[prop(into)] on_render: Callback<()>,
) -> impl IntoView {
    let html = RwSignal::new(String::new());
    let pending = StoredValue::new(String::new());
    let scheduled = StoredValue::new(false);
    Effect::new(move |_| {
        pending.set_value(text.get());
        if !scheduled.get_value() {
            scheduled.set_value(true);
            set_timeout(
                move || {
                    if let Some(text) = pending.try_get_value() {
                        html.try_set(render_markdown(&text));
                        scheduled.try_set_value(false);
                        on_render.run(());
                    }
                },
                Duration::from_millis(100),
            );
        }
    });
    view! { <div class="markdown" inner_html=move || html.get()></div> }
}

use wasm_bindgen::JsCast;
