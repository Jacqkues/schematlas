//! The installed harness catalog is separate from chat and ACP transport.
use super::HarnessLogo;
use crate::{
    api,
    components::{
        dialogs::{FormError, Modal},
        icons::Icon,
    },
    types::{AgentSnapshot, InstalledAgent},
};
use leptos::{prelude::*, task::spawn_local};

const PRESET_KEY: &str = "atlas:agent-preset";

#[component]
pub(super) fn AgentChooser(
    snapshot: RwSignal<Option<AgentSnapshot>>,
    busy: RwSignal<bool>,
    choosing_name: RwSignal<String>,
    on_select: Callback<InstalledAgent>,
    on_disconnect: Callback<()>,
) -> impl IntoView {
    let agents = RwSignal::new(Vec::<InstalledAgent>::new());
    let scanning = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let open = RwSignal::new(false);
    let root = NodeRef::<leptos::html::Div>::new();
    super::dismiss_on_outside(root, open);
    let custom = RwSignal::new(false);
    let saved = api::local_storage_get(PRESET_KEY)
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
    let previous = RwSignal::new(
        saved
            .as_ref()
            .and_then(|s| s["executable"].as_str())
            .unwrap_or_default()
            .to_owned(),
    );
    let custom_args = saved
        .as_ref()
        .and_then(|s| s["args"].as_str())
        .unwrap_or("[]")
        .to_owned();
    let custom_args = StoredValue::new(custom_args);
    let connected = Memo::new(move |_| {
        snapshot.with(|s| {
            s.as_ref()
                .is_some_and(|s| !matches!(s.status.as_str(), "disconnected" | "error"))
        })
    });
    let locked = move || {
        busy.get()
            || snapshot.with(|s| {
                s.as_ref().is_some_and(|s| {
                    matches!(s.status.as_str(), "running" | "cancelling" | "connecting")
                        || !s.reviews.is_empty()
                })
            })
    };
    let expanded = move || open.get();
    Effect::watch(
        move || connected.get(),
        move |connected, _, _| {
            if *connected {
                open.set(false);
            }
        },
        false,
    );
    let scan = Callback::new(move |()| {
        if scanning.get_untracked() {
            return;
        }
        scanning.set(true);
        error.set(String::new());
        spawn_local(async move {
            match api::discover_agents().await {
                Ok(mut list) => {
                    let preferred = previous.try_get_untracked().unwrap_or_default();
                    list.sort_by_key(|a| (!a.available, a.executable != preferred, a.name.clone()));
                    let _ = agents.try_set(list);
                }
                Err(e) => {
                    let _ = error.try_set(e);
                }
            }
            let _ = scanning.try_set(false);
        });
    });
    scan.run(());
    let choose = Callback::new(move |agent: InstalledAgent| {
        if locked() || !agent.available {
            return;
        }
        previous.set(agent.executable.clone());
        custom_args.set_value(serde_json::to_string(&agent.args).unwrap_or_default());
        api::local_storage_set(PRESET_KEY, &serde_json::json!({"executable": agent.executable, "args": serde_json::to_string(&agent.args).unwrap_or_default()}).to_string());
        open.set(false);
        custom.set(false);
        on_select.run(agent);
    });
    view! {
        <div node_ref=root class="relative min-w-0 shrink-0" on:focusout=move |ev| super::dismiss_picker(root, open, ev) on:keydown=move |ev| { if ev.key() == "Escape" { open.set(false); } }>
            <button type="button" class="flex min-h-8 max-w-[180px] items-center gap-1.5 rounded-lg px-2 text-left text-xs hover:bg-surface disabled:cursor-default" aria-label="Choose agent" aria-expanded=expanded disabled=locked on:click=move |_| open.update(|v| *v = !*v)>
                <HarnessLogo name=Signal::derive(move || snapshot.with(|s| s.as_ref().map(|s| s.agent_name.clone()).unwrap_or_default())) />
                <span class="min-w-0 flex-1 truncate font-medium">{move || if busy.get() { choosing_name.get() } else { snapshot.with(|s| s.as_ref().filter(|_| connected.get()).map(|s| s.agent_name.clone())).unwrap_or_else(|| "Choose agent".into()) }}</span>

                <Icon name="chevron-down" size=13 class="text-muted" />
            </button>
            <Show when=expanded>
                <section aria-label="Installed agents" class="absolute bottom-full left-0 z-30 mb-2 max-h-[50vh] w-64 overflow-auto rounded-xl border border-line-strong bg-surface p-2 shadow-xl">
                    <div class="mb-1 flex items-center justify-between px-2">
                        <span class="text-[10px] text-muted">"On this computer"</span>
                        <button type="button" class="icon-btn size-6" aria-label="Refresh agents" title="Refresh agents" disabled=move || scanning.get() || locked() on:click=move |_| scan.run(())><Icon name="refresh-cw" size=12 /></button>
                    </div>
                    <Show when=move || scanning.get()><p class="px-2 py-3 text-xs text-muted" role="status">"Finding your agents…"</p></Show>
                    <Show when=move || !scanning.get() && agents.with(Vec::is_empty)>
                        <p class="px-2 py-3 text-xs leading-5 text-muted">"No agents found. Install a coding agent, then refresh."</p>
                    </Show>
                    <For each=move || agents.get() key=|a| (a.id.clone(), a.executable.clone()) children=move |agent| {
                        let ready = agent.available;
                        let launch = agent.clone();
                        let exe = agent.executable.clone();
                        let current = Memo::new(move |_| connected.get() && snapshot.with(|s| s.as_ref().is_some_and(|s| s.executable == exe)));
                        let logo_name = StoredValue::new(agent.name.clone());
                        view! {
                            <div class="flex items-center gap-1 rounded-lg hover:bg-surface-3">
                                <button type="button" class="flex min-h-14 min-w-0 flex-1 items-center gap-3 rounded-lg px-2 py-2 text-left disabled:cursor-default disabled:text-muted" disabled=move || !ready || locked() aria-label=format!("Use {}", agent.name) aria-pressed=move || current.get().to_string() on:click=move |_| choose.run(launch.clone())>
                                    <span class="flex size-8 shrink-0 items-center justify-center"><HarnessLogo name=Signal::derive(move || logo_name.get_value()) /></span>
                                    <span class="min-w-0 flex-1"><span class="block truncate text-xs font-medium">{agent.name.clone()}</span><span class="mt-0.5 block text-[10px] text-muted">{move || if current.get() { "Connected" } else if ready { "Installed" } else { "Unavailable" }}</span></span>
                                    <Show when=move || current.get()><Icon name="check" size=14 class="text-accent-text" /></Show>
                                </button>
                            </div>
                        }
                    } />
                    <FormError error=error />
                    <div class="mt-2 flex items-center justify-between border-t border-line-soft pt-2">
                        <button type="button" class="rounded-md px-2 py-1.5 text-[11px] text-muted hover:bg-surface-3 hover:text-ink" disabled=locked on:click=move |_| custom.set(true)>"Custom agent…"</button>
                        <Show when=move || connected.get()><button type="button" class="rounded-md px-2 py-1.5 text-[11px] text-muted hover:bg-surface-3 hover:text-ink" disabled=locked on:click=move |_| { open.set(false); on_disconnect.run(()); }>"Disconnect"</button></Show>
                    </div>
                </section>
            </Show>
        </div>
        <Show when=move || custom.get()>
            <CustomAgent executable=previous.get_untracked() args=custom_args.get_value() on_connect=choose on_close=Callback::new(move |()| custom.set(false)) />
        </Show>
    }
}

#[component]
fn CustomAgent(
    executable: String,
    args: String,
    on_connect: Callback<InstalledAgent>,
    on_close: Callback<()>,
) -> impl IntoView {
    let executable = RwSignal::new(executable);
    let args = RwSignal::new(args);
    let error = RwSignal::new(String::new());
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let raw = args.get_untracked();
        let parsed = if raw.trim().is_empty() {
            Ok(vec![])
        } else {
            serde_json::from_str::<Vec<String>>(&raw)
        };
        match parsed {
            Ok(args) => on_connect.run(InstalledAgent {
                name: "Custom agent".into(),
                executable: executable.get_untracked().trim().into(),
                args,
                available: true,
                ..Default::default()
            }),
            Err(_) => error.set("Use a JSON array of arguments, or leave it empty.".into()),
        }
    };
    view! {
        <Modal title="Custom agent" on_close=on_close>
            <form on:submit=submit>
                <p class="mb-5 text-xs leading-5 text-muted">"Connect another ACP-compatible executable. It will use this project’s folder."</p>
                <label class="form-label" for="custom-agent-executable">"Executable"</label>
                <div class="mb-5 flex gap-2"><input id="custom-agent-executable" class="field min-w-0 flex-1 font-mono text-xs" placeholder="/absolute/path/to/agent" bind:value=executable required autofocus /><button type="button" class="icon-btn" aria-label="Choose agent executable" on:click=move |_| spawn_local(async move { match api::open_dialog("Agent executable", false, None).await { Ok(Some(path)) => { let _ = executable.try_set(path); }, Ok(None) => {}, Err(e) => { let _ = error.try_set(e); } } })><Icon name="folder-open" size=16 /></button></div>
                <label class="form-label" for="custom-agent-args">"Arguments (optional)"</label>
                <input id="custom-agent-args" class="field mb-5 font-mono text-xs" placeholder=r#"["--acp"]"# bind:value=args />
                <FormError error=error />
                <div class="modal-footer"><button type="button" class="btn" on:click=move |_| on_close.run(())>"Cancel"</button><button type="submit" class="btn btn-primary" disabled=move || executable.with(|s| s.trim().is_empty())>"Connect"</button></div>
            </form>
        </Modal>
    }
}
