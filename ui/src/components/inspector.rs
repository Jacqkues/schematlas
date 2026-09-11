//! Right-hand drawer with the selected table's columns and relationships.
use super::icons::Icon;
use crate::types::{Entity, Source};
use leptos::prelude::*;
use std::sync::Arc;

#[component]
pub fn Inspector(
    entity: Entity,
    source: Arc<Source>,
    #[prop(into)] on_close: Callback<()>,
    #[prop(into)] on_select: Callback<Entity>,
) -> impl IntoView {
    let database = source.is_database();
    let relations: Vec<(Entity, String)> = source
        .graph
        .relations
        .iter()
        .filter(|r| r.source == entity.id || r.target == entity.id)
        .filter_map(|r| {
            let other_id = if r.source == entity.id { &r.target } else { &r.source };
            let other = source.graph.entities.iter().find(|e| &e.id == other_id)?.clone();
            let label = match (&r.source_field, &r.target_field) {
                (Some(s), Some(t)) => format!("{s} → {t}"),
                _ => r.label.clone(),
            };
            Some((other, label))
        })
        .collect();
    let badge = |text: String| {
        view! { <span class="rounded-[3px] border border-line px-1 py-0.5 font-mono text-[7px] text-[#c0c2c5]">{text}</span> }
    };
    let title = format!("{}{}", entity.method.as_ref().map(|m| format!("{m} ")).unwrap_or_default(), entity.name);
    view! {
        <aside class="inspector w-[300px] shrink-0 animate-rise-in overflow-y-auto border-l border-line-soft bg-sidebar max-[1180px]:w-[270px] max-[1000px]:absolute max-[1000px]:inset-y-0 max-[1000px]:right-0 max-[1000px]:z-[8] max-[1000px]:shadow-[-6px_0_20px_#0006]">
            <div class="flex items-center justify-between border-b border-line-soft px-4 py-3">
                <span class="eyebrow">"INSPECTOR"</span>
                <button type="button" class="icon-btn" aria-label="Close inspector" on:click=move |_| on_close.run(())><Icon name="x" size=17 /></button>
            </div>
            <div class="border-b border-line-soft px-5 py-[23px]">
                <div class="tile"><Icon name="box" size=23 /></div>
                <span class="eyebrow mt-5 mb-[7px] block text-[8px] tracking-[0.7px]">{format!("{} / {}", entity.namespace, entity.kind)}</span>
                <h2 class="font-mono text-[19px] font-medium tracking-[-0.6px] [overflow-wrap:anywhere]">{title}</h2>
                {entity.description.clone().filter(|d| !d.is_empty()).map(|d| view! {
                    <p class="mt-[13px] text-[11px] leading-[1.8] whitespace-pre-wrap text-[#b4b6b9] [overflow-wrap:anywhere]">{d}</p>
                })}
            </div>
            <div class="border-b border-line-soft px-4 py-[22px]">
                <div class="section-heading mx-1 mb-[15px] text-[9px]">
                    {if database { "COLUMNS" } else { "FIELDS" }}
                    <span class="font-mono tracking-normal">{entity.fields.len()}</span>
                </div>
                {entity.fields.iter().map(|field| {
                    let mut badges = Vec::new();
                    if field.primary_key { badges.push("PRIMARY KEY".to_string()); }
                    if database {
                        badges.push(if field.nullable { "NULLABLE" } else { "NOT NULL" }.to_string());
                    } else {
                        if field.required { badges.push("REQUIRED".into()); }
                        if field.nullable { badges.push("NULLABLE".into()); }
                    }
                    let data_type = if field.data_type.is_empty() { "any".to_string() } else { field.data_type.clone() };
                    view! {
                        <div class="border-t border-line-soft px-1 py-3">
                            <div class="flex items-start justify-between gap-3">
                                <strong class="flex gap-1.5 font-mono text-[11px] font-normal text-soft [overflow-wrap:anywhere]">
                                    {field.primary_key.then(|| view! { <Icon name="key-round" size=12 /> })}
                                    {field.name.clone()}
                                </strong>
                                <code class="max-w-[130px] text-right text-[10px] text-soft [overflow-wrap:anywhere]">{data_type}</code>
                            </div>
                            <div class="mt-[7px] flex flex-wrap gap-[5px]">{badges.into_iter().map(badge).collect_view()}</div>
                            {field.default_value.clone().map(|value| view! {
                                <p class="mt-[7px] text-[10px] leading-relaxed text-[#b4b6b9] [overflow-wrap:anywhere]">"Default: " <code>{value}</code></p>
                            })}
                            {field.description.clone().filter(|d| !d.is_empty()).map(|d| view! {
                                <p class="mt-[7px] text-[10px] leading-relaxed text-[#b4b6b9] [overflow-wrap:anywhere]">{d}</p>
                            })}
                        </div>
                    }
                }).collect_view()}
            </div>
            <div class="border-b border-line-soft px-4 py-[22px]">
                <div class="section-heading mx-1 mb-[15px] text-[9px]">
                    "RELATIONSHIPS" <span class="font-mono tracking-normal">{relations.len()}</span>
                </div>
                {if relations.is_empty() {
                    view! { <p class="form-hint">"No relationships for this node."</p> }.into_any()
                } else {
                    relations.into_iter().map(|(other, label)| {
                        let name = other.name.clone();
                        view! {
                            <button
                                type="button"
                                class="flex w-full items-center gap-[9px] px-1 py-2.5 text-left text-[#c7c9cc] transition-colors hover:bg-surface"
                                on:click=move |_| on_select.run(other.clone())
                            >
                                <Icon name="link-2" size=14 />
                                <div class="min-w-0 flex-1">
                                    <strong class="block font-mono text-[10px] font-normal [overflow-wrap:anywhere]">{name}</strong>
                                    <small class="mt-1 block font-mono text-[8px] text-soft [overflow-wrap:anywhere]">{label}</small>
                                </div>
                                <Icon name="arrow-up-right" size=14 />
                            </button>
                        }
                    }).collect_view().into_any()
                }}
            </div>
        </aside>
    }
}
