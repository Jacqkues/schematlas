<script lang="ts">
  import { fly } from 'svelte/transition';
  import { X, KeyRound, ArrowUpRight, Box, Link2 } from '@lucide/svelte';
  import { motion } from '$lib/motion';
  import type { Entity, Source } from '$lib/types';
  let {
    entity,
    source,
    onclose,
    onselect,
  }: { entity: Entity; source: Source; onclose: () => void; onselect: (entity: Entity) => void } =
    $props();
  let relations = $derived(
    source.graph.relations.filter((r) => r.source === entity.id || r.target === entity.id),
  );
</script>

{#snippet badge(text: string)}
  <span class="rounded-[3px] border border-line px-1 py-0.5 font-mono text-[7px] text-text"
    >{text}</span
  >
{/snippet}

<aside
  class="inspector w-[300px] shrink-0 overflow-y-auto border-l border-line-soft bg-sidebar max-[1180px]:w-[270px] max-[1000px]:absolute max-[1000px]:inset-y-0 max-[1000px]:right-0 max-[1000px]:z-[8] max-[1000px]:shadow-[-6px_0_20px_#0006]"
  transition:fly={motion.drawer()}
>
  <div class="flex items-center justify-between border-b border-line-soft px-4 py-3">
    <span class="eyebrow">INSPECTOR</span><button
      class="icon-btn"
      aria-label="Close inspector"
      onclick={onclose}><X size={17} /></button
    >
  </div>
  <div class="border-b border-line-soft px-5 py-[23px]">
    <div class="tile"><Box size={23} /></div>
    <span class="mt-5 mb-[7px] block eyebrow text-[8px] tracking-[0.7px]"
      >{entity.namespace} / {entity.kind}</span
    >
    <h2 class="font-mono text-[19px] font-medium tracking-[-0.6px] [overflow-wrap:anywhere]">
      {entity.method ? `${entity.method} ` : ''}{entity.name}
    </h2>
    {#if entity.description}<p
        class="mt-[13px] text-[11px] leading-[1.8] [overflow-wrap:anywhere] whitespace-pre-wrap text-soft"
      >
        {entity.description}
      </p>{/if}
  </div>
  <div class="border-b border-line-soft px-4 py-[22px]">
    <div class="mx-1 mb-[15px] section-heading text-[9px]">
      {source.kind === 'database' ? 'COLUMNS' : 'FIELDS'}<span class="font-mono tracking-normal"
        >{entity.fields.length}</span
      >
    </div>
    {#each entity.fields as field}<div class="border-t border-line-soft px-1 py-3">
        <div class="flex items-start justify-between gap-3">
          <strong
            class="flex gap-1.5 font-mono text-[11px] font-normal [overflow-wrap:anywhere] text-soft"
            >{#if field.primaryKey}<KeyRound size={12} />{/if}{field.name}</strong
          ><code class="max-w-[130px] text-right text-[10px] [overflow-wrap:anywhere] text-soft"
            >{field.dataType || 'any'}</code
          >
        </div>
        <div class="mt-[7px] flex flex-wrap gap-[5px]">
          {#if field.primaryKey}{@render badge(
              'PRIMARY KEY',
            )}{/if}{#if source.kind === 'database'}{@render badge(
              field.nullable ? 'NULLABLE' : 'NOT NULL',
            )}{:else}{#if field.required}{@render badge(
                'REQUIRED',
              )}{/if}{#if field.nullable}{@render badge('NULLABLE')}{/if}{/if}
        </div>
        {#if field.defaultValue !== null}<p
            class="mt-[7px] text-[10px] leading-relaxed [overflow-wrap:anywhere] text-soft"
          >
            Default: <code>{field.defaultValue}</code>
          </p>{/if}{#if field.description}<p
            class="mt-[7px] text-[10px] leading-relaxed [overflow-wrap:anywhere] text-soft"
          >
            {field.description}
          </p>{/if}
      </div>{/each}
  </div>
  <div class="border-b border-line-soft px-4 py-[22px]">
    <div class="mx-1 mb-[15px] section-heading text-[9px]">
      RELATIONSHIPS<span class="font-mono tracking-normal">{relations.length}</span>
    </div>
    {#each relations as relation}{@const otherId =
        relation.source === entity.id ? relation.target : relation.source}{@const other =
        source.graph.entities.find((e) => e.id === otherId)}{#if other}<button
          class="flex w-full items-center gap-[9px] px-1 py-2.5 text-left text-text transition-colors hover:bg-surface"
          onclick={() => onselect(other)}
          ><Link2 size={14} />
          <div class="min-w-0 flex-1">
            <strong class="block font-mono text-[10px] font-normal [overflow-wrap:anywhere]"
              >{other.name}</strong
            ><small class="mt-1 block font-mono text-[8px] [overflow-wrap:anywhere] text-soft"
              >{relation.sourceField && relation.targetField
                ? `${relation.sourceField} → ${relation.targetField}`
                : relation.label}</small
            >
          </div>
          <ArrowUpRight size={14} /></button
        >{/if}{:else}<p class="form-hint">No relationships for this node.</p>{/each}
  </div>
</aside>
