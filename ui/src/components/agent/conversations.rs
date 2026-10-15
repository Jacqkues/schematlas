use super::HarnessLogo;
use crate::{
    api,
    components::icons::Icon,
    types::{AgentSnapshot, ConversationSummary},
};
use leptos::{prelude::*, task::spawn_local};

#[component]
pub fn ConversationList(
    project_id: String,
    agent_open: RwSignal<bool>,
    /// Drawn as a bordered column that scrolls on its own, for the one-screen overview.
    #[prop(optional)]
    panel: bool,
) -> impl IntoView {
    let workspace = expect_context::<crate::state::Workspace>();
    let revision = RwSignal::new(0_usize);
    let rows = RwSignal::new(Vec::<ConversationSummary>::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let project = StoredValue::new(project_id.clone());
    let refresh = Callback::new(move |()| {
        let id = project.get_value();
        revision.update(|v| *v += 1);
        let request = revision.get_untracked();
        spawn_local(async move {
            let result = api::agent_conversations(&id).await;
            if revision.try_get_untracked() != Some(request) {
                return;
            }
            match result {
                Ok(list) => {
                    let _ = rows.try_set(list);
                }
                Err(e) => {
                    let _ = error.try_set(e);
                }
            }
        });
    });
    refresh.run(());
    let listener = api::listen::<AgentSnapshot>("agent:update", move |event| {
        if event.project_id == project_id
            && !matches!(event.status.as_str(), "running" | "cancelling")
        {
            refresh.run(());
        }
    });
    let _keep = StoredValue::new_local(listener);
    let reopen = Callback::new(move |row: ConversationSummary| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(String::new());
        let id = project.get_value();
        spawn_local(async move {
            match api::agent_reopen_conversation(&id, &row.id).await {
                Ok(()) => {
                    if workspace.project_id.get_untracked().as_deref() == Some(id.as_str()) {
                        agent_open.set(true);
                    }
                }
                Err(e) => {
                    let _ = error.try_set(e);
                }
            }
            let _ = busy.try_set(false);
        });
    });
    view! {
        <section class=if panel { "flex min-h-0 flex-col overflow-hidden rounded-2xl border border-line bg-surface" } else { "mb-9" } aria-label="Conversations">
            <div class=if panel { "flex h-12 shrink-0 items-center gap-2 border-b border-line-soft px-4" } else { "mb-4 flex items-center justify-between" }>
                {panel.then(|| view! { <Icon name="message-square" size=15 class="text-muted" /> })}
                <h2 class=if panel { "text-[13px] font-medium" } else { "text-sm font-medium" }>"Conversations"</h2>
                <span class=if panel { "ml-auto text-[11px] text-muted" } else { "text-[11px] text-muted" }>{move || format!("{} saved with your agents", rows.with(Vec::len))}</span>
            </div>
            <Show when=move || !rows.with(Vec::is_empty) fallback=move || view! { <p class=if panel { "p-4 text-xs text-muted" } else { "text-xs text-muted" }>"Start a chat above. You can return to it here."</p> }>
                <div class=if panel { "min-h-0 flex-1 overflow-auto overscroll-contain" } else { "overflow-hidden rounded-xl border border-line bg-surface" }>
                    <For each=move || rows.get() key=|r| r.id.clone() children=move |row| {
                        let id = row.id.clone();
                        let current = Memo::new(move |_| rows.with(|rows| rows.iter().find(|r| r.id == id).cloned().unwrap_or_else(|| row.clone())));
                        view! { <button type="button" class="flex min-h-16 w-full items-center gap-3 border-b border-line-soft px-4 py-3 text-left last:border-b-0 hover:bg-surface-3 disabled:cursor-default" aria-label=move || current.with(|r| format!("Open {} with {}", r.label, r.agent_name)) disabled=move || busy.get() || current.with(|r| !r.available) on:click=move |_| reopen.run(current.get_untracked())>
                            <HarnessLogo name=Signal::derive(move || current.with(|r| r.agent_name.clone())) />
                            <span class="min-w-0 flex-1"><span class="block truncate text-xs font-medium">{move || current.with(|r| r.label.clone())}</span><span class="mt-1 block text-[11px] text-muted">{move || current.with(|r| r.agent_name.clone())}</span></span>
                            <span class="text-[10px] text-muted">{move || current.with(|r| if !r.available { "Agent unavailable" } else if r.active { "Current" } else { "Reopen" })}</span>
                            <Icon name="arrow-up-right" size=14 class="text-muted" />
                        </button> }
                    } />
                </div>
            </Show>
            <Show when=move || !error.get().is_empty()><p class="form-error mt-3" role="alert">{move || error.get()}</p></Show>
        </section>
    }
}
