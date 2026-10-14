//! The team channel: presence, a shared feed of chat, comments and change notes, and the opt-in
//! that starts syncing through the repository's `schematlas-sync` branch.
use super::dialogs::FormError;
use super::icons::Icon;
use crate::api;
use crate::state::{AtlasItem, Workspace};
use crate::types::{Author, SyncEvent, SyncPreview, SyncSnapshot};
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

/// Comments on a diagram that nobody has resolved yet.
pub fn open_comments<'a>(events: &'a [SyncEvent], diagram: &str) -> Vec<&'a SyncEvent> {
    let resolved: HashSet<&str> = events
        .iter()
        .filter(|e| e.kind == "comment_resolve")
        .filter_map(|e| e.body["commentId"].as_str())
        .collect();
    events
        .iter()
        .filter(|e| {
            e.kind == "comment"
                && e.body["diagram"].as_str() == Some(diagram)
                && !resolved.contains(e.id.as_str())
        })
        .collect()
}
/// Open comment counts per node of one diagram, for the card badges.
pub fn comment_counts(snapshot: Option<&SyncSnapshot>, diagram: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    if let Some(snapshot) = snapshot {
        for comment in open_comments(&snapshot.events, diagram) {
            if let Some(node) = comment.body["node"].as_str() {
                *counts.entry(node.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
}
pub fn sync_enabled(workspace: Workspace) -> bool {
    workspace.sync.with_untracked(|s| {
        s.as_ref()
            .is_some_and(|s| s.enabled && s.status != "confirm")
    })
}
/// Post to the channel when it is on; the channel is optional, so failures stay quiet.
pub fn announce(workspace: Workspace, kind: &'static str, body: Value) {
    if !sync_enabled(workspace) {
        return;
    }
    let Some(project_id) = workspace.project_id.get_untracked() else {
        return;
    };
    spawn_local(async move {
        if let Ok(snapshot) = api::sync_post(&project_id, kind, body).await {
            if workspace.project_id.get_untracked().as_deref() == Some(snapshot.project_id.as_str())
            {
                workspace.sync.set(Some(snapshot));
            }
        }
    });
}

fn initials(name: &str) -> String {
    let letters: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect();
    if letters.is_empty() {
        "?".into()
    } else {
        letters.to_uppercase()
    }
}
/// A stable hue per teammate.
fn hue(key: &str) -> u32 {
    key.bytes()
        .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32))
        % 360
}
#[component]
pub fn Avatar(author: Author, #[prop(default = 26)] size: u32) -> impl IntoView {
    view! {
        <span
            class="inline-flex shrink-0 items-center justify-center rounded-full font-semibold text-white"
            style:width=format!("{size}px")
            style:height=format!("{size}px")
            style:font-size=format!("{}px", size * 2 / 5)
            style:background=format!("hsl({} 38% 42%)", hue(&author.key))
            title=author.name.clone()
        >
            {initials(&author.name)}
        </span>
    }
}

fn time_of(ts: &str) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(ts));
    if date.get_time().is_nan() {
        return String::new();
    }
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"hour".into(), &"2-digit".into());
    let _ = js_sys::Reflect::set(&options, &"minute".into(), &"2-digit".into());
    let today = js_sys::Date::new_0().to_date_string() == date.to_date_string();
    let time = String::from(date.to_locale_time_string_with_options("default", &options));
    if today {
        time
    } else {
        format!(
            "{} {time}",
            String::from(date.to_locale_date_string("default", &wasm_bindgen::JsValue::UNDEFINED))
        )
    }
}

fn diagram_title(workspace: Workspace, slug: &str) -> String {
    workspace.repo.with_untracked(|r| {
        r.diagrams
            .iter()
            .find(|d| d.slug == slug)
            .map(|d| d.title.clone())
            .unwrap_or_else(|| slug.to_string())
    })
}

#[component]
fn FeedItem(event: SyncEvent, resolved: bool) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let diagram = event.body["diagram"].as_str().map(str::to_string);
    let node = event.body["node"].as_str().map(str::to_string);
    let text = event.body["text"].as_str().unwrap_or_default().to_string();
    let open = diagram.clone().map(|slug| {
        let title = diagram_title(workspace, &slug);
        let target = slug.clone();
        view! {
            <button
                type="button"
                class="inline-flex items-center gap-1 rounded bg-surface-4 px-1.5 py-0.5 font-mono text-[10px] text-accent-text hover:bg-accent-soft"
                on:click=move |_| workspace.open_item(AtlasItem::Diagram(target.clone()))
            >
                <Icon name="workflow" size=10 /> {title}
            </button>
        }
    });
    let body = match event.kind.as_str() {
        "chat" => view! { <p class="mt-1 text-xs leading-relaxed whitespace-pre-wrap text-ink [overflow-wrap:anywhere]">{text}</p> }.into_any(),
        "comment" => {
            let id = event.id.clone();
            view! {
                <div class="mt-1 rounded-md border border-line bg-surface-3 p-2" class=("opacity-60", resolved)>
                    <div class="mb-1 flex flex-wrap items-center gap-1.5 text-[10px] text-muted">
                        <Icon name="message-square" size=11 /> "commented on " {open}
                        {node.clone().map(|n| view! { <span class="font-mono text-faint">{format!("› {n}")}</span> })}
                    </div>
                    <p class="text-xs leading-relaxed whitespace-pre-wrap text-ink [overflow-wrap:anywhere]">{text}</p>
                    {(!resolved).then(|| view! {
                        <button type="button" class="mt-1.5 text-[10px] text-muted hover:text-accent" on:click=move |_| announce(workspace, "comment_resolve", json!({"commentId": id.clone()}))>
                            "Resolve"
                        </button>
                    })}
                </div>
            }.into_any()
        }
        "diagram_changed" => {
            let summary = event.body["summary"].as_str().unwrap_or("updated").to_string();
            let commit = event.body["commit"].as_str().map(|c| c.chars().take(8).collect::<String>());
            view! {
                <p class="mt-1 flex flex-wrap items-center gap-1.5 text-xs text-text">
                    <Icon name="git-branch" size=12 class="text-sage" /> {summary} {open}
                    {commit.map(|c| view! { <code class="font-mono text-[10px] text-faint">{c}</code> })}
                </p>
            }.into_any()
        }
        "comment_resolve" => view! { <p class="mt-1 text-[11px] text-muted">"resolved a comment"</p> }.into_any(),
        _ => ().into_any(),
    };
    view! {
        <li class="flex gap-2.5 py-2.5">
            <Avatar author=event.author.clone() />
            <div class="min-w-0 flex-1">
                <div class="flex items-baseline gap-2">
                    <strong class="text-xs font-semibold text-ink">{event.author.name.clone()}</strong>
                    <time class="text-[10px] text-faint">{time_of(&event.ts)}</time>
                </div>
                {body}
            </div>
        </li>
    }
}

#[component]
fn SetupSync(project_id: String) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let preview = RwSignal::new(None::<SyncPreview>);
    let remote = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let load = {
        let project_id = project_id.clone();
        move |choice: Option<String>| {
            let project_id = project_id.clone();
            spawn_local(async move {
                busy.set(true);
                error.set(String::new());
                match api::sync_preview(&project_id, choice.as_deref()).await {
                    Ok(p) => {
                        remote.set(p.remote.clone().unwrap_or_default());
                        preview.set(Some(p));
                    }
                    Err(e) => error.set(e),
                }
                busy.set(false);
            });
        }
    };
    let confirm = {
        let project_id = project_id.clone();
        move |_| {
            let project_id = project_id.clone();
            spawn_local(async move {
                busy.set(true);
                error.set(String::new());
                match api::sync_enable(&project_id, &remote.get_untracked()).await {
                    Ok(snapshot) => workspace.sync.set(Some(snapshot)),
                    Err(e) => error.set(e),
                }
                busy.set(false);
            });
        }
    };
    let reload = load.clone();
    view! {
        <div class="flex flex-col gap-3 p-4 text-xs leading-relaxed text-text">
            <div class="tile"><Icon name="users" size=20 /></div>
            <h3 class="text-sm font-semibold text-ink">"Work on this map together"</h3>
            <p>
                "Team sync shares presence, comments and change notes with everyone who works on this repository. "
                "It travels through your own git remote on a separate "
                <code class="font-mono text-accent-text">"schematlas-sync"</code>
                " branch, so no extra service or account is involved."
            </p>
            {move || match preview.get() {
                None => {
                    let load = load.clone();
                    view! {
                        <button type="button" class="btn btn-primary self-start" disabled=move || busy.get() on:click=move |_| load(None)>
                            {move || if busy.get() { "Checking the repository…" } else { "Set up team sync" }}
                        </button>
                    }.into_any()
                }
                Some(p) => {
                    let reload = reload.clone();
                    let confirm = confirm.clone();
                    let ready = p.error.is_none() && p.name.is_some() && p.has_email && p.remote.is_some();
                    view! {
                        <dl class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-2 rounded-md border border-line bg-surface-3 p-3 text-[11px]">
                            <dt class="text-muted">"Remote"</dt>
                            <dd>
                                <select class="field py-1" on:change=move |ev| { let value = event_target_value(&ev); reload(Some(value)); }>
                                    {p.remotes.iter().map(|r| view! { <option value=r.clone() selected=Some(r) == p.remote.as_ref()>{r.clone()}</option> }).collect_view()}
                                </select>
                            </dd>
                            <dt class="text-muted">"Address"</dt>
                            <dd class="font-mono break-all">{p.remote_url.clone().unwrap_or_default()}</dd>
                            <dt class="text-muted">"Branch"</dt>
                            <dd class="font-mono">{format!("{}{}", p.branch, if p.branch_exists { " (already shared)" } else { " (will be created)" })}</dd>
                            <dt class="text-muted">"You appear as"</dt>
                            <dd>{p.name.clone().unwrap_or_else(|| "— set git user.name".into())}</dd>
                        </dl>
                        <p class="rounded-md border border-accent-line bg-accent-soft p-2.5 text-[11px] text-accent-text">
                            "Everyone with access to this repository can read what you post. Don’t share secrets in comments. Commits on this branch are marked [skip ci]."
                        </p>
                        {(!p.has_email).then(|| view! { <p class="text-[11px] text-warning">"Set git user.email in this repository first (it is never published, only a key derived from it)."</p> })}
                        {p.error.clone().map(|e| view! { <p class="form-error mt-0 whitespace-pre-wrap">{e}</p> })}
                        <button type="button" class="btn btn-primary self-start" disabled=move || busy.get() || !ready on:click=confirm>
                            {move || if busy.get() { "Starting…" } else { "Share through this remote" }}
                        </button>
                    }.into_any()
                }
            }}
            <FormError error=error />
        </div>
    }
}

#[component]
pub fn TeamPanel(project_id: String, #[prop(into)] on_close: Callback<()>) -> impl IntoView {
    let workspace = expect_context::<Workspace>();
    let draft = RwSignal::new(String::new());
    let menu = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let feed = NodeRef::<leptos::html::Ol>::new();
    let desktop = api::desktop();
    {
        let project_id = project_id.clone();
        spawn_local(async move {
            if desktop {
                if let Ok(snapshot) = api::sync_now(&project_id).await {
                    workspace.sync.set(Some(snapshot));
                }
            }
        });
    }
    let snapshot = move || workspace.sync.get();
    let enabled = move || snapshot().is_some_and(|s| s.enabled);
    let events = Memo::new(move |_| {
        workspace.sync.with(|s| {
            s.as_ref()
                .map(|s| {
                    let resolved: HashSet<String> = s
                        .events
                        .iter()
                        .filter(|e| e.kind == "comment_resolve")
                        .filter_map(|e| e.body["commentId"].as_str().map(str::to_string))
                        .collect();
                    s.events
                        .iter()
                        .filter(|e| {
                            matches!(e.kind.as_str(), "chat" | "comment" | "diagram_changed")
                        })
                        .map(|e| (e.clone(), resolved.contains(&e.id)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    Effect::new(move |_| {
        events.track();
        if let Some(list) = feed.get() {
            list.set_scroll_top(list.scroll_height());
        }
    });
    let send = {
        let project_id = project_id.clone();
        move || {
            let text = draft.get_untracked().trim().to_string();
            if text.is_empty() {
                return;
            }
            let project_id = project_id.clone();
            spawn_local(async move {
                match api::sync_post(&project_id, "chat", json!({ "text": text })).await {
                    Ok(snapshot) => {
                        draft.set(String::new());
                        error.set(String::new());
                        workspace.sync.set(Some(snapshot));
                    }
                    Err(e) => error.set(e),
                }
            });
        }
    };
    let send_key = send.clone();
    let disable = {
        let project_id = project_id.clone();
        move |delete_remote: bool| {
            let project_id = project_id.clone();
            spawn_local(async move {
                match api::sync_disable(&project_id, delete_remote).await {
                    Ok(snapshot) => {
                        menu.set(false);
                        workspace.sync.set(Some(snapshot));
                    }
                    Err(e) => error.set(e),
                }
            });
        }
    };
    let sync_now = {
        let project_id = project_id.clone();
        move |_| {
            let project_id = project_id.clone();
            spawn_local(async move {
                match api::sync_now(&project_id).await {
                    Ok(snapshot) => workspace.sync.set(Some(snapshot)),
                    Err(e) => error.set(e),
                }
            });
        }
    };
    let status = move || {
        snapshot()
            .map(|s| match s.status.as_str() {
                "syncing" | "connecting" => ("Syncing…".to_string(), "bg-warning"),
                "error" => ("Sync problem".to_string(), "bg-danger-line"),
                "confirm" => ("Needs confirmation".to_string(), "bg-warning"),
                _ => (
                    match s.last_sync.as_deref() {
                        Some(ts) => format!("Synced {}", time_of(ts)),
                        None => "Waiting".to_string(),
                    },
                    "bg-accent",
                ),
            })
            .unwrap_or(("Off".into(), "bg-muted"))
    };
    let disable_keep = disable.clone();
    view! {
        <section class="flex h-full min-w-0 flex-1 flex-col bg-surface" aria-label="Team">
            <header class="flex min-h-[58px] items-center gap-2.5 border-b border-line px-4">
                <Icon name="users" size=17 class="text-sage" />
                <h2 class="text-sm font-semibold">"Team"</h2>
                <Show when=enabled>
                    <span class="flex items-center gap-1.5 text-[10px] text-muted">
                        <span class=move || format!("inline-block size-1.5 rounded-full {}", status().1)></span>
                        {move || status().0}
                        <Show when=move || snapshot().is_some_and(|s| s.pending > 0)>
                            <span class="text-faint">{move || format!("· {} to send", snapshot().map(|s| s.pending).unwrap_or(0))}</span>
                        </Show>
                    </span>
                </Show>
                <div class="ml-auto flex items-center gap-1">
                    <Show when=enabled>
                        <button type="button" class="icon-btn" aria-label="Sync now" title="Sync now" on:click=sync_now.clone()><Icon name="refresh-cw" size=15 /></button>
                        <button type="button" class="icon-btn" aria-label="Team sync settings" aria-pressed=move || menu.get().to_string() on:click=move |_| menu.update(|m| *m = !*m)><Icon name="ellipsis" size=16 /></button>
                    </Show>
                    <button type="button" class="icon-btn" aria-label="Close team panel" on:click=move |_| on_close.run(())><Icon name="x" size=16 /></button>
                </div>
            </header>
            <Show when=move || menu.get()>
                <div class="flex flex-col gap-2 border-b border-line bg-surface-3 p-3 text-[11px] text-text">
                    <p class="font-mono text-[10px] break-all text-muted">{move || snapshot().and_then(|s| s.remote_url).unwrap_or_default()}</p>
                    <button type="button" class="btn justify-start" on:click={let d = disable_keep.clone(); move |_| d(false)}>"Stop syncing on this computer"</button>
                    <button type="button" class="btn btn-ghost-danger justify-start" on:click={let d = disable.clone(); move |_| d(true)}>"Stop and delete the shared branch"</button>
                </div>
            </Show>
            {move || if !desktop {
                view! {
                    <div class="flex flex-col gap-3 p-4 text-xs leading-relaxed text-muted">
                        <div class="tile"><Icon name="users" size=20 /></div>
                        <p>"Team sync runs in the desktop app. It shares presence, comments and change notes through your repository’s git remote."</p>
                    </div>
                }.into_any()
            } else if !enabled() {
                view! { <SetupSync project_id=project_id.clone() /> }.into_any()
            } else {
                let send_click = send.clone();
                let send_key = send_key.clone();
                view! {
                    <Show when=move || snapshot().is_some_and(|s| s.error.is_some())>
                        <p class="form-error mx-4 mt-3 whitespace-pre-wrap">{move || snapshot().and_then(|s| s.error).unwrap_or_default()}</p>
                    </Show>
                    <div class="border-b border-line-soft px-4 py-3">
                        <div class="eyebrow mb-2">"HERE NOW"</div>
                        <ul class="flex flex-col gap-1.5">
                            <For
                                each=move || snapshot().map(|s| s.presence).unwrap_or_default()
                                key=|p| (p.author.key.clone(), p.ts.clone(), p.diagram.clone())
                                children=move |p| {
                                    let me = snapshot().and_then(|s| s.me).is_some_and(|m| m.key == p.author.key);
                                    let place = p.diagram.clone().map(|slug| diagram_title(workspace, &slug));
                                    view! {
                                        <li class="flex items-center gap-2 text-[11px]">
                                            <Avatar author=p.author.clone() size=20 />
                                            <span class="text-ink">{p.author.name.clone()}{me.then_some(" (you)")}</span>
                                            {place.map(|t| view! { <span class="truncate text-muted">{format!("· on {t}")}</span> })}
                                        </li>
                                    }
                                }
                            />
                        </ul>
                    </div>
                    <ol node_ref=feed class="min-h-0 flex-1 divide-y divide-line-soft overflow-auto px-4" aria-label="Team activity" aria-live="polite">
                        <Show when=move || events.with(|e| e.is_empty())>
                            <li class="py-8 text-center text-[11px] text-muted">"No activity yet. Say hello, or comment on a node from its inspector."</li>
                        </Show>
                        <For each=move || events.get() key=|(e, resolved)| (e.id.clone(), *resolved) children=|(event, resolved)| view! { <FeedItem event=event resolved=resolved /> } />
                    </ol>
                    <form class="flex items-end gap-2 border-t border-line p-3" on:submit=move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); send_click(); }>
                        <textarea
                            class="field max-h-32 min-h-[38px] resize-none"
                            rows="1"
                            aria-label="Message the team"
                            placeholder="Message the team…"
                            bind:value=draft
                            on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                                if ev.key() == "Enter" && !ev.shift_key() {
                                    ev.prevent_default();
                                    send_key();
                                }
                            }
                        ></textarea>
                        <button type="submit" class="btn btn-primary px-2.5" aria-label="Send"><Icon name="send" size=15 /></button>
                    </form>
                    <FormError error=error />
                }.into_any()
            }}
        </section>
    }
}
