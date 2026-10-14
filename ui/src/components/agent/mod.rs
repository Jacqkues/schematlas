//! Local coding agent panel: ACP session controls, transcript, reviews and discovery.
mod chooser;
mod conversations;
use crate::api;
use crate::components::frame_value::FrameValue;
use crate::components::icons::Icon;
use crate::markdown::render_markdown;
use crate::types::{AgentSnapshot, InstalledAgent, Review, DATABASE_KINDS};
use chooser::AgentChooser;
pub use conversations::ConversationList;
use leptos::prelude::*;
use leptos::task::spawn_local;
use std::time::Duration;

const CHAT_WIDTH_KEY: &str = "atlas.chat.width";
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
    let actual = Memo::new(move |_| width.get().round().clamp(MINIMUM, maximum.get()));
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
    let resize = FrameValue::new(move |x: f64| {
        if let Some((start, w)) = origin.get_value() {
            width.set((w + start - x).clamp(MINIMUM, maximum.get_untracked()));
        }
    });
    let moving = move |ev: leptos::ev::PointerEvent| {
        if origin.get_value().is_some() {
            resize.push(ev.client_x() as f64);
        }
    };
    let stop = move |ev: leptos::ev::PointerEvent| {
        if origin.get_value().is_some() {
            resize.push(ev.client_x() as f64);
            resize.flush();
            origin.set_value(None);
            dragging.set(false);
            save();
        }
    };
    let cancel = move |_ev: leptos::ev::PointerEvent| {
        resize.cancel();
        if let Some((_, w)) = origin.try_update_value(Option::take).flatten() {
            width.set(w);
            dragging.set(false);
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
            class="relative flex min-h-0 shrink-0 [&>.agent-panel]:w-(--chat-width) [&>.agent-panel]:shrink-0 max-[1100px]:absolute max-[1100px]:inset-y-0 max-[1100px]:right-0 max-[1100px]:z-30 max-[1100px]:shadow-[-10px_0_30px_#0005]"
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
                on:pointercancel=cancel
                on:lostpointercapture=cancel
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

/// Session states are backend vocabulary; the panel shows what they mean to the user.
/// An ACP stop reason as the sentence shown after the turn. `end_turn` and
/// `cancelled` never reach here: finishing needs no notice, and cancelling
/// already has its own feedback.
fn stop_notice(reason: &str) -> &'static str {
    match reason {
        "max_tokens" => "The agent reached its token limit, so this answer stops mid-way.",
        "max_turn_requests" => {
            "The agent reached its limit of steps for one turn, so this answer is unfinished."
        }
        "refusal" => {
            "The agent declined to continue. It will not see this prompt, or anything after it, on your next question."
        }
        _ => "The agent stopped before finishing, so this answer may be incomplete.",
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
    #[prop(default = false)] inline: bool,
    #[prop(default = Callback::new(|()| {}), into)] on_start: Callback<()>,
) -> impl IntoView {
    let desktop = api::desktop();
    let snapshot = RwSignal::new(None::<AgentSnapshot>);
    let choosing_name = RwSignal::new(String::new());
    let workspace = expect_context::<crate::state::Workspace>();
    let prompt = workspace.agent_draft;
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
    let ready = move || status.get() == "ready" && !busy.get() && !workspace.agent_starting.get();
    let resumed = move || snapshot.with(|s| s.as_ref().is_some_and(|s| s.resumed));
    let stop_reason = move || snapshot.with(|s| s.as_ref().and_then(|s| s.stop_reason.clone()));
    let running = move || matches!(status.get().as_str(), "running" | "cancelling");

    let near_bottom = move || {
        transcript
            .try_get_untracked()
            .flatten()
            .map(|el| (el.scroll_height() - el.scroll_top() - el.client_height()) < 100)
            .unwrap_or(true)
    };
    let show_latest = move |smooth: bool| {
        request_animation_frame(move || {
            if let Some(el) = transcript.try_get_untracked().flatten() {
                let options = web_sys::ScrollToOptions::new();
                options.set_top(el.scroll_height() as f64);
                options.set_behavior(if smooth {
                    web_sys::ScrollBehavior::Smooth
                } else {
                    web_sys::ScrollBehavior::Instant
                });
                el.scroll_to_with_scroll_to_options(&options);
            }
            let _ = unread.try_set(false);
        });
    };
    if desktop {
        let listener_project = project_id.clone();
        let listener = api::listen::<AgentSnapshot>("agent:update", move |event| {
            if event.project_id != listener_project {
                return;
            }
            let bottom = near_bottom();
            let _ = snapshot.try_set(Some(event));
            if bottom {
                show_latest(false);
            } else {
                let _ = unread.try_set(true);
            }
        });
        let _keep = StoredValue::new_local(listener);
        let id = project_id.clone();
        spawn_local(async move {
            match api::agent_status(&id).await {
                Ok(value) => {
                    if snapshot.try_get_untracked().is_some_and(|s| s.is_none()) {
                        let _ = snapshot.try_set(value);
                        show_latest(false);
                    }
                }
                Err(e) => {
                    let _ = error.try_set(e);
                }
            }
        });
    }

    let perform = {
        let project_id = project_id.clone();
        move |task: AgentTask| {
            if let AgentTask::Connect(agent) = &task {
                if busy.get_untracked()
                    || matches!(
                        status.get_untracked().as_str(),
                        "running" | "cancelling" | "connecting"
                    )
                {
                    return;
                }
                busy.set(true);
                choosing_name.set(agent.name.clone());
            }
            let project_id = project_id.clone();
            spawn_local(async move {
                let _ = error.try_set(String::new());
                let result = match task {
                    AgentTask::Connect(agent) => {
                        let result = if agent.id.is_empty() {
                            match api::agent_working_directory(&project_id).await {
                                Ok(cwd) => {
                                    api::agent_connect(
                                        &project_id,
                                        &agent.executable,
                                        &agent.args,
                                        &cwd,
                                    )
                                    .await
                                }
                                Err(e) => Err(e),
                            }
                        } else {
                            api::agent_connect_installed(&project_id, &agent.id).await
                        };
                        if let Ok(value) = api::agent_status(&project_id).await {
                            let _ = snapshot.try_set(value);
                        }
                        let _ = busy.try_set(false);
                        result
                    }
                    AgentTask::Prompt(text, fresh) => {
                        let reset = if fresh {
                            api::agent_new_conversation(&project_id).await
                        } else {
                            Ok(())
                        };
                        if workspace.project_id.get_untracked().as_deref()
                            == Some(project_id.as_str())
                        {
                            workspace.agent_starting.set(false);
                        }
                        let result = match reset {
                            Ok(()) => api::agent_prompt(&project_id, &text).await,
                            Err(e) => Err(e),
                        };
                        if result.is_err()
                            && workspace.project_id.get_untracked().as_deref()
                                == Some(project_id.as_str())
                        {
                            prompt.update(|draft| {
                                if draft.is_empty() {
                                    *draft = text;
                                } else {
                                    *draft = format!("{text}\n\n{draft}");
                                }
                            });
                        }
                        result
                    }
                    AgentTask::Model(id) => {
                        busy.set(true);
                        let result = api::agent_set_model(&project_id, &id).await;
                        let _ = busy.try_set(false);
                        result
                    }
                    AgentTask::Cancel => api::agent_cancel(&project_id).await,
                    AgentTask::NewConversation => api::agent_new_conversation(&project_id).await,
                    AgentTask::Disconnect => {
                        let result = api::agent_disconnect(&project_id).await;
                        if result.is_ok() {
                            let _ = snapshot.try_set(None);
                        }
                        result
                    }
                    AgentTask::Decide(review_id, option) => {
                        api::agent_decide(&project_id, &review_id, option.as_deref()).await
                    }
                    AgentTask::Authenticate(method) => {
                        busy.set(true);
                        let r = api::agent_authenticate(&project_id, &method).await;
                        let _ = busy.try_set(false);
                        r
                    }
                };
                if let Err(e) = result {
                    let _ = error.try_set(e);
                }
            });
        }
    };
    let perform = StoredValue::new_local(perform);
    let run = move |task: AgentTask| perform.with_value(|p| p(task));
    let send = move || {
        let text = prompt.get_untracked();
        if text.trim().is_empty() || !ready() {
            return;
        }
        prompt.set(String::new());
        let fresh = inline
            && snapshot.with_untracked(|s| {
                s.as_ref()
                    .is_some_and(|s| !s.messages.is_empty() || s.resumed)
            });
        if inline {
            workspace.agent_starting.set(true);
        }
        run(AgentTask::Prompt(text, fresh));
        if inline {
            on_start.run(());
        }
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
    let agent_name =
        move || snapshot.with(|s| s.as_ref().map(|s| s.agent_name.clone()).unwrap_or_default());

    view! {
        <aside class=if inline { "flex min-h-0 flex-col text-left text-ink" } else { "agent-panel flex min-h-0 flex-col border-l border-line-soft bg-surface text-ink" } aria-label="Local coding agent">
            <Show when=move || !inline><header class="flex h-14 shrink-0 items-center gap-2 px-4">
                <Icon name="sparkles" size=16 class="text-muted" />
                <h2 class="text-sm font-medium">"Chat"</h2>
                <span class="min-w-0 flex-1 truncate text-xs text-muted">{project_name.clone()}</span>
                <button type="button" class="icon-btn size-7" aria-label="New conversation" title="New conversation" disabled=move || !ready() || busy.get() on:click=move |_| run(AgentTask::NewConversation)><Icon name="plus" size=15 /></button>
                <button type="button" class="icon-btn size-7" aria-label="Close agent panel" on:click=move |_| on_close.run(())><Icon name="x" size=17 /></button>
            </header></Show>
            <Show when=move || busy.get() || status.get() == "connecting">
                <p class="px-4 py-2 text-xs text-muted" role="status">{move || format!("Connecting to {}…", choosing_name.get())}</p>
            </Show>
                    <Show when=move || !inline && resumed()>
                        <p class="border-y border-line px-5 py-2 text-[10px] leading-relaxed text-soft">
                            "Conversation resumed."
                        </p>
                    </Show>
                    <Show when=move || status.get() == "authentication">
                        <div class="p-[22px]">
                            <p class="text-xs leading-relaxed text-soft">"Authenticate with your agent to start a session."</p>
                            {move || snapshot.with(|s| s.as_ref().map(|s| s.auth_methods.clone()).unwrap_or_default()).into_iter().map(|method| {
                                let id = method.id.clone();
                                view! { <button type="button" class="btn" disabled=move || busy.get() on:click=move |_| run(AgentTask::Authenticate(id.clone()))>{method.name.clone()}</button> }
                            }).collect_view()}
                        </div>
                    </Show>
                    <Show when=move || workspace.agent_starting.get()><p class="px-4 py-2 text-xs text-muted" role="status">"Starting conversation…"</p></Show>
                    <Show when=move || !inline && !workspace.agent_starting.get()>
                    {move || snapshot.get().map(|s| view! { <AgentActivity snapshot=s /> })}
                    <div
                        node_ref=transcript
                        class=if inline { "max-h-[50vh] overflow-auto overscroll-contain px-4" } else { "min-h-[100px] flex-1 overflow-auto overscroll-contain px-[18px] pt-2 pb-5" }
                        role="log"
                        aria-label="Agent conversation"
                        aria-live="polite"
                        on:scroll=move |_| { if near_bottom() { let _ = unread.try_set(false); } }
                    >
                        <Show when=move || !inline && snapshot.with(|s| s.as_ref().is_none_or(|s| s.messages.is_empty()))>
                            <div class="px-2 py-8 text-center text-muted">
                                <p class="text-sm">{move || if connected.get() { "What would you like to understand?" } else { "Choose an agent, then ask about your project." }}</p>
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
                                let call_status = move || current.with(|m| m.status.clone().unwrap_or_default());
                                let text = Signal::derive(move || current.with(|m| m.text.clone()));
                                let body = Signal::derive(move || {
                                    current.with(|m| m.detail.clone().unwrap_or_else(|| m.text.clone()))
                                });
                                let first_line = move || text.with(|text| text.lines().next().unwrap_or_default().to_string());
                                view! {
                                    <article
                                        data-tool-status=move || { let state = call_status(); (!state.is_empty()).then_some(state) }
                                        class="my-[18px] [contain-intrinsic-size:auto_100px] [content-visibility:auto]"
                                        class=("rounded-[10px]", user) class=("border", user) class=("border-line-soft", user) class=("bg-surface", user) class=("px-3.5", user) class=("py-3", user)
                                    >
                                        <span class="flex items-center gap-2 text-[10px] font-semibold text-muted">
                                            {who}
                                            {move || {
                                                let state = call_status();
                                                (!state.is_empty()).then(|| {
                                                    let tone = match state.as_str() {
                                                        "failed" => "border-danger-line text-danger-text",
                                                        "completed" => "border-line text-muted",
                                                        _ => "border-accent-line text-accent-text",
                                                    };
                                                    let label = match state.as_str() {
                                                        "pending" | "in_progress" => "running",
                                                        "completed" => "done",
                                                        other => other,
                                                    };
                                                    view! {
                                                        <span class=format!("rounded-full border px-1.5 py-px text-[9px] font-medium tracking-wide uppercase {tone}")>{label.to_string()}</span>
                                                    }
                                                })
                                            }}
                                        </span>
                                        {if tool {
                                            view! {
                                                <details>
                                                    <summary class="mt-1.5 cursor-pointer truncate font-mono text-[11px] leading-relaxed text-soft">{first_line}</summary>
                                                    <p class=format!("{} max-h-[200px] overflow-auto font-mono text-[11px] text-soft", message_text_class())>{move || body.get()}</p>
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
                    {move || stop_reason().map(|reason| view! {
                        <p class="mx-4 mb-3 rounded-lg border border-accent-line bg-surface px-3 py-2.5 text-[11px] leading-relaxed text-soft" role="status">
                            {stop_notice(&reason)}
                        </p>
                    })}
                    <Show when=move || snapshot.with(|s| s.as_ref().is_some_and(|s| !s.reviews.is_empty()))>
                        <section class="max-h-[40%] shrink-0 overflow-auto border-t border-line px-4 pb-3" aria-label="Pending approvals">
                            <h3 class="sticky top-0 z-10 -mx-4 flex items-center gap-2 border-b border-line bg-surface px-4 py-2.5 text-[11px] font-semibold text-ink">
                                <Icon name="shield-check" size=14 />
                                {move || { let n = snapshot.with(|s| s.as_ref().map_or(0, |s| s.reviews.len())); format!("{n} request{} waiting for your approval", if n == 1 { "" } else { "s" }) }}
                            </h3>
                            <For each=move || snapshot.with(|s| s.as_ref().map(|s| s.reviews.clone()).unwrap_or_default()) key=|r| r.id.clone() children=move |review| {
                                let review_id = review.id.clone();
                                view! { <ReviewCard review=review on_decide=Callback::new(move |option: Option<String>| run(AgentTask::Decide(review_id.clone(), option))) /> }
                            } />
                        </section>
                    </Show>
                    </Show>
                    <form class="mx-4 mb-4 rounded-[22px] border border-line bg-surface-3 p-4 focus-within:border-accent focus-within:ring-2 focus-within:ring-accent/25" on:submit=move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); send(); }>
                        <label class="sr-only" for="agent-prompt">"Message your agent"</label>
                        <textarea
                            id="agent-prompt"
                            class="max-h-[200px] w-full resize-y border-0 bg-transparent text-xs leading-relaxed text-ink outline-none focus-visible:outline-none"
                            bind:value=prompt
                            rows="3"
                            maxlength="65536"
                            placeholder=if inline { "Ask about your codebase…" } else { "Ask about this project…" }
                            aria-describedby="agent-composer-help"
                            on:keydown=composer_keydown
                        ></textarea>
                        <div class="flex items-center justify-between gap-2">
                            <div class="flex min-w-0 items-center gap-1">
                                {if desktop { view! {
                                    <AgentChooser snapshot=snapshot busy=busy choosing_name=choosing_name on_select=Callback::new(move |agent| run(AgentTask::Connect(agent))) on_disconnect=Callback::new(move |()| run(AgentTask::Disconnect)) />
                                    <ModelChooser snapshot=snapshot disabled=Signal::derive(move || !ready() || snapshot.with(|s| s.as_ref().is_some_and(|s| !s.reviews.is_empty()))) on_select=Callback::new(move |id| run(AgentTask::Model(id))) />
                                }.into_any() } else { view! { <span class="text-xs text-muted">"Browser preview · Desktop agents only"</span> }.into_any() }}
                            </div>
                            {move || if running() {
                                view! { <button type="button" class="btn" on:click=move |_| run(AgentTask::Cancel)><Icon name="square" size=13 /> "Stop"</button> }.into_any()
                            } else {
                                view! {
                                    <button type="submit" class="btn btn-primary size-9 min-h-0 rounded-full p-0" aria-label="Send message" disabled=move || !ready() || prompt.with(|p| p.trim().is_empty())>
                                        <Icon name="arrow-up" size=16 />
                                    </button>
                                }.into_any()
                            }}
                        </div>
                            <span id="agent-composer-help" class="text-[10px] text-soft">
                                {move || if !connected.get() { "Choose an agent to send" } else if running() { "Working…" } else { "Enter sends · Shift+Enter for a new line" }}
                            </span>
                    </form>
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
    Connect(InstalledAgent),
    Prompt(String, bool),
    Model(String),
    Cancel,
    NewConversation,
    Disconnect,
    Decide(String, Option<String>),
    Authenticate(String),
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

/// Payloads come from the agent, so a field only counts when it is a non-blank string.
fn detail_text(details: &serde_json::Value, key: &str) -> Option<String> {
    details
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Engines we know read better by name; anything else keeps the agent's own wording.
fn database_label(kind: &str) -> String {
    DATABASE_KINDS
        .iter()
        .find(|(id, _)| *id == kind)
        .map_or_else(|| kind.to_string(), |(_, label)| (*label).to_string())
}

fn detail_field(label: &'static str, value: String) -> impl IntoView {
    view! {
        <div class="min-w-0">
            <dt class="eyebrow">{label}</dt>
            <dd class="mt-[3px] text-[11px] leading-relaxed text-ink [overflow-wrap:anywhere]">{value}</dd>
        </div>
    }
}

#[component]
fn ReviewCard(review: Review, #[prop(into)] on_decide: Callback<Option<String>>) -> impl IntoView {
    let busy = RwSignal::new(false);
    let decide = move |option: Option<String>| {
        busy.set(true);
        on_decide.run(option);
    };
    let code_class = "mt-2.5 max-h-[220px] overflow-auto rounded-md border border-accent-line bg-field p-2.5 font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-ink [overflow-wrap:anywhere]";
    let payload = serde_json::to_string_pretty(&review.details).unwrap_or_default();
    // What is about to run has to be readable on its own; keys stay as labelled text, never JSON.
    let statement = match review.kind.as_str() {
        "sql" => detail_text(&review.details, "sql").map(|sql| {
            let source = detail_text(&review.details, "source");
            let database = detail_text(&review.details, "databaseKind").map(|k| database_label(&k));
            let labelled = source.is_some() || database.is_some();
            let note = detail_text(&review.details, "note");
            view! {
                <pre class=code_class><code class="font-mono">{sql}</code></pre>
                {labelled.then(|| view! {
                    <dl class="mt-2.5 flex flex-wrap gap-x-5 gap-y-2">
                        {source.map(|value| detail_field("SOURCE", value))}
                        {database.map(|value| detail_field("DATABASE", value))}
                    </dl>
                })}
                {note.map(|value| view! {
                    <p class="mt-2.5 flex items-start gap-1.5 text-[10px] leading-relaxed text-warning">
                        <Icon name="circle-alert" size=12 />
                        {value}
                    </p>
                })}
            }.into_any()
        }),
        "http" => detail_text(&review.details, "url").map(|url| {
            let method = detail_text(&review.details, "method");
            let authentication = detail_text(&review.details, "authentication");
            let body = review
                .details
                .get("body")
                .filter(|value| !value.is_null())
                .map(|value| match value.as_str() {
                    Some(text) => text.to_string(),
                    None => serde_json::to_string_pretty(value)
                        .unwrap_or_else(|_| value.to_string()),
                });
            view! {
                <p class="mt-2.5 flex flex-wrap items-baseline gap-2">
                    {method.map(|value| view! {
                        <span class="rounded border border-accent-line bg-surface px-1.5 py-[3px] font-mono text-[10px] font-semibold tracking-wide text-accent-text uppercase">{value}</span>
                    })}
                    <span class="min-w-0 flex-1 font-mono text-[11px] leading-relaxed text-ink [overflow-wrap:anywhere]">{url}</span>
                </p>
                {authentication.map(|value| view! {
                    <dl class="mt-2.5">{detail_field("AUTHENTICATION", value)}</dl>
                })}
                {body.map(|value| view! {
                    <>
                        <span class="eyebrow mt-3 block">"BODY"</span>
                        <pre class=code_class><code class="font-mono">{value}</code></pre>
                    </>
                })}
            }.into_any()
        }),
        "diagram" => detail_text(&review.details, "path").map(|path| {
            let summary = detail_text(&review.details, "summary").unwrap_or_default();
            let new = review.details["isNew"].as_bool().unwrap_or(false);
            let change = review.details["change"].clone();
            let list = |key: &str| -> Vec<String> {
                change[key]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                    .unwrap_or_default()
            };
            let groups = [
                ("+", "Added nodes", list("addedNodes")),
                ("~", "Changed nodes", list("changedNodes")),
                ("−", "Removed nodes", list("removedNodes")),
                ("+", "Added connections", list("addedEdges")),
                ("~", "Changed connections", list("changedEdges")),
                ("−", "Removed connections", list("removedEdges")),
            ];
            view! {
                <p class="mt-2.5 flex flex-wrap items-center gap-2">
                    <span class="rounded border border-accent-line bg-surface px-1.5 py-[3px] font-mono text-[10px] font-semibold text-accent-text">{if new { "NEW" } else { "UPDATE" }}</span>
                    <span class="font-mono text-[11px] text-ink [overflow-wrap:anywhere]">{path}</span>
                </p>
                <p class="mt-2 font-mono text-[11px] text-text">{summary}</p>
                <div class="mt-2 flex flex-col gap-2">
                    {groups.into_iter().filter(|(_, _, items)| !items.is_empty()).map(|(sign, label, items)| view! {
                        <div>
                            <span class="eyebrow block">{label.to_uppercase()}</span>
                            <ul class="mt-1 flex flex-col gap-0.5 text-[11px] text-ink">
                                {items.into_iter().take(40).map(|item| view! { <li class="[overflow-wrap:anywhere]"><span class="mr-1.5 font-mono text-faint">{sign}</span>{item}</li> }).collect_view()}
                            </ul>
                        </div>
                    }).collect_view()}
                </div>
            }.into_any()
        }),
        "doc" => detail_text(&review.details, "path").map(|path| {
            let diff = detail_text(&review.details, "diff").unwrap_or_default();
            let new = review.details["isNew"].as_bool().unwrap_or(false);
            view! {
                <p class="mt-2.5 flex flex-wrap items-center gap-2">
                    <span class="rounded border border-accent-line bg-surface px-1.5 py-[3px] font-mono text-[10px] font-semibold text-accent-text">{if new { "NEW" } else { "UPDATE" }}</span>
                    <span class="font-mono text-[11px] text-ink [overflow-wrap:anywhere]">{path}</span>
                </p>
                <pre class=code_class>
                    {diff.lines().map(|line| {
                        let class = if line.starts_with('+') && !line.starts_with("+++") {
                            "text-accent"
                        } else if line.starts_with('-') && !line.starts_with("---") {
                            "text-danger-text"
                        } else {
                            "text-muted"
                        };
                        view! { <div class=class>{line.to_string()}</div> }
                    }).collect_view()}
                </pre>
            }.into_any()
        }),
        _ => None,
    };
    let request = match statement {
        // Nothing is ever hidden: what we did not lay out stays one disclosure away.
        Some(statement) => view! {
            {statement}
            <details class="mt-3">
                <summary class="cursor-pointer text-[10px] text-muted transition-colors hover:text-ink">"Full request"</summary>
                <pre class=code_class>{payload}</pre>
            </details>
        }
        .into_any(),
        // Shapes we do not know keep the payload itself visible.
        None => view! { <pre class=code_class>{payload}</pre> }.into_any(),
    };
    view! {
        <section class="my-[18px] rounded-[9px] border border-accent-line bg-accent-soft p-3.5" aria-label="Permission request">
            <span class="eyebrow text-muted">{format!("REVIEW REQUIRED · {}", review.kind)}</span>
            <h4 class="mt-[5px] text-[13px] font-semibold [overflow-wrap:anywhere]">{review.title.clone()}</h4>
            {request}
            <div class="mt-3.5 flex flex-wrap gap-1.5 border-t border-accent-line pt-3">
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

#[component]
fn HarnessLogo(name: Signal<String>) -> impl IntoView {
    view! { <span class="inline-flex size-4 shrink-0 items-center justify-center [&>svg]:size-full" aria-hidden="true" inner_html=move || {
        let name = name.get().to_lowercase();
        if name.contains("claude") { include_str!("logos/claude.svg") }
        else if name.contains("codex") || name.contains("openai") { include_str!("logos/openai.svg") }
        else if name.contains("gemini") { include_str!("logos/gemini.svg") }
        else if name.contains("opencode") { include_str!("logos/opencode.svg") }
        else { r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="3" y="4" width="18" height="16" rx="3"/><path d="m7 9 3 3-3 3m6 0h4"/></svg>"# }
    } /> }
}

#[component]
fn ModelChooser(
    snapshot: RwSignal<Option<AgentSnapshot>>,
    disabled: Signal<bool>,
    on_select: Callback<String>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let root = NodeRef::<leptos::html::Div>::new();
    dismiss_on_outside(root, open);
    let models =
        move || snapshot.with(|s| s.as_ref().map(|s| s.models.clone()).unwrap_or_default());
    let label = move || {
        snapshot.with(|s| {
            s.as_ref()
                .and_then(|s| {
                    s.models
                        .iter()
                        .find(|m| {
                            Some(&m.id) == s.model_id.as_ref()
                                || (s.model_id.is_some() && m.resolved_id == s.model_id)
                        })
                        .map(|m| m.name.clone())
                        .or_else(|| s.model_id.clone())
                })
                .unwrap_or_else(|| "Default model".into())
        })
    };
    view! {
        <div node_ref=root class="relative min-w-0" on:focusout=move |ev| dismiss_picker(root, open, ev) on:keydown=move |ev| { if ev.key() == "Escape" { open.set(false); } }>
            <button type="button" class="flex min-h-8 max-w-[180px] items-center gap-1 rounded-lg px-2 text-xs text-soft hover:bg-surface disabled:text-faint" aria-label="Choose model" aria-expanded=move || open.get() disabled=move || disabled.get() || models().is_empty() title=move || if models().is_empty() { "This agent uses its default model" } else { "Choose model" } on:click=move |_| open.update(|v| *v = !*v)><span class="truncate">{label}</span><Icon name="chevron-down" size=12 /></button>
            <Show when=move || open.get() && !disabled.get()>
                <section aria-label="Available models" class="absolute bottom-full right-0 z-30 mb-2 max-h-[45vh] w-64 overflow-auto rounded-xl border border-line-strong bg-surface p-2 shadow-xl">
                    <p class="px-2 py-1 text-[10px] text-muted">"Models"</p>
                    <For each=models key=|m| m.id.clone() children=move |model| {
                        let id = model.id.clone();
                        let current = Memo::new(move |_| snapshot.with(|s| s.as_ref().is_some_and(|s| s.model_id.as_ref() == Some(&id) || (s.model_id.is_some() && s.models.iter().any(|m| m.id == id && m.resolved_id == s.model_id)))));
                        view! { <button type="button" class="flex min-h-9 w-full items-center justify-between gap-2 rounded-lg px-2 text-left text-xs hover:bg-surface-3" aria-pressed=move || current.get().to_string() on:click=move |_| { open.set(false); on_select.run(model.id.clone()); }><span>{model.name}</span><Show when=move || current.get()><Icon name="check" size=13 /></Show></button> }
                    } />
                </section>
            </Show>
        </div>
    }
}

fn dismiss_picker(
    root: NodeRef<leptos::html::Div>,
    open: RwSignal<bool>,
    event: leptos::ev::FocusEvent,
) {
    use wasm_bindgen::JsCast;
    let inside = event
        .related_target()
        .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
        .is_some_and(|target| {
            root.get()
                .is_some_and(|element| element.contains(Some(&target)))
        });
    if !inside {
        open.set(false);
    }
}

fn dismiss_on_outside(root: NodeRef<leptos::html::Div>, open: RwSignal<bool>) {
    use wasm_bindgen::JsCast;
    let outside = move |target: Option<web_sys::EventTarget>| {
        if !open.get_untracked() {
            return;
        }
        let inside = target
            .and_then(|target| target.dyn_into::<web_sys::Node>().ok())
            .is_some_and(|target| {
                root.get_untracked()
                    .is_some_and(|element| element.contains(Some(&target)))
            });
        if !inside {
            open.set(false);
        }
    };
    let pointer = window_event_listener(leptos::ev::pointerdown, move |ev| outside(ev.target()));
    let focus = window_event_listener(leptos::ev::focusin, move |ev| outside(ev.target()));
    on_cleanup(move || {
        pointer.remove();
        focus.remove();
    });
}
