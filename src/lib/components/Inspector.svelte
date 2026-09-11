<script lang="ts">
  import { X, KeyRound, ArrowUpRight, Box, Link2 } from '@lucide/svelte';
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

<aside class="inspector">
  <div class="inspector-top">
    <span class="eyebrow">INSPECTOR</span><button
      class="icon-button"
      aria-label="Close inspector"
      onclick={onclose}><X size={17} /></button
    >
  </div>
  <div class="inspector-title">
    <div
      class="tile-icon"
      class:sage={source.kind === 'openapi'}
      class:terracotta={source.kind === 'database'}
    >
      <Box size={23} />
    </div>
    <span class="eyebrow">{entity.namespace} / {entity.kind}</span>
    <h2>{entity.method ? `${entity.method} ` : ''}{entity.name}</h2>
    {#if entity.description}<p>{entity.description}</p>{/if}
  </div>
  <div class="inspector-section">
    <div class="section-heading">
      {source.kind === 'database' ? 'COLUMNS' : 'FIELDS'}<span>{entity.fields.length}</span>
    </div>
    {#each entity.fields as field}<div class="inspector-field">
        <div>
          <strong
            >{#if field.primaryKey}<KeyRound size={12} />{/if}{field.name}</strong
          ><code>{field.dataType || 'any'}</code>
        </div>
        <div class="field-badges">
          {#if field.primaryKey}<span>PRIMARY KEY</span>{/if}{#if source.kind === 'database'}<span
              >{field.nullable ? 'NULLABLE' : 'NOT NULL'}</span
            >{:else}{#if field.required}<span>REQUIRED</span>{/if}{#if field.nullable}<span
                >NULLABLE</span
              >{/if}{/if}
        </div>
        {#if field.defaultValue !== null}<p class="field-default">
            Default: <code>{field.defaultValue}</code>
          </p>{/if}{#if field.description}<p>{field.description}</p>{/if}
      </div>{/each}
  </div>
  <div class="inspector-section">
    <div class="section-heading">RELATIONSHIPS<span>{relations.length}</span></div>
    {#each relations as relation}{@const otherId =
        relation.source === entity.id ? relation.target : relation.source}{@const other =
        source.graph.entities.find((e) => e.id === otherId)}{#if other}<button
          class="relation-link"
          onclick={() => onselect(other)}
          ><Link2 size={14} />
          <div>
            <strong>{other.name}</strong><small
              >{relation.sourceField && relation.targetField
                ? `${relation.sourceField} → ${relation.targetField}`
                : relation.label}</small
            >
          </div>
          <ArrowUpRight size={14} /></button
        >{/if}{:else}<p class="form-hint">No relationships for this node.</p>{/each}
  </div>
</aside>
