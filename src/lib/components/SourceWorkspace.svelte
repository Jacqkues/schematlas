<script lang="ts">
  import { fly } from 'svelte/transition';
  import { SvelteFlowProvider } from '@xyflow/svelte';
  import GraphCanvas from './graph/GraphCanvas.svelte';
  import CanvasGroups from './graph/CanvasGroups.svelte';
  import Inspector from './Inspector.svelte';
  import SchemaFilter from './SchemaFilter.svelte';
  import {
    Search,
    Database,
    Braces,
    RefreshCw,
    Download,
    Trash2,
    PlugZap,
    Layers3,
    Group,
    Ellipsis,
    X,
  } from '@lucide/svelte';
  import { motion } from '$lib/motion';
  import { databaseNames, type Entity, type Project, type Source, type Position } from '$lib/types';
  let {
    project,
    source,
    onupdate,
    onapi,
    onrefresh,
    onreconnect,
    onexport,
    onremove,
    onsave,
    refreshing,
  }: {
    project: Project;
    source: Source;
    onupdate: (p: Project) => void;
    onapi: () => void;
    onrefresh: () => void;
    onreconnect: () => void;
    onexport: () => void;
    onremove: () => void;
    onsave: (positions: Record<string, Position>) => void;
    refreshing: boolean;
  } = $props();
  type PanelName = 'groups' | 'filters' | 'details';
  const panels: Record<PanelName, { title: string; label: string }> = {
    groups: { title: 'Groups', label: 'Manage groups' },
    filters: { title: 'Schemas', label: 'Schema filters' },
    details: { title: 'Source details', label: 'Source details' },
  };
  const navButton =
    'flex items-center gap-1.5 rounded-md border border-transparent px-[9px] py-2 text-[11px] text-text transition-colors hover:border-accent-line hover:bg-accent-soft hover:text-accent-text aria-expanded:border-accent-line aria-expanded:bg-accent-soft aria-expanded:text-accent-text';
  let query = $state('');
  let namespaces = $state<string[]>([]);
  let related = $state(true);
  let selected = $state<Entity | null>(null);
  let panel = $state<PanelName | null>(null);
  let focusNodeIds = $state<string[]>([]);
  function togglePanel(next: PanelName) {
    selected = null;
    panel = panel === next ? null : next;
    if (next === 'filters') focusNodeIds = [];
  }
  function focusGroup(ids: string[]) {
    query = '';
    namespaces = [];
    focusNodeIds = [...ids];
    panel = null;
  }
</script>

<div class="flex h-full min-h-0 animate-fade-in flex-col">
  <header
    class="flex min-h-[58px] shrink-0 items-center gap-[18px] border-b border-line bg-surface px-[18px] max-[1100px]:gap-2 max-[1100px]:px-3"
  >
    <div class="flex min-w-0 flex-1 items-center gap-[9px] text-sage">
      {#if source.kind === 'database'}<Database size={17} />{:else}<Braces size={17} />{/if}
      <h1 class="truncate text-sm font-semibold text-ink">{source.name}</h1>
    </div>
    <search
      class="flex w-[clamp(140px,20vw,260px)] items-center gap-2 text-faint max-[1100px]:w-40"
    >
      <Search size={14} /><input
        class="w-full border-0 bg-transparent py-[9px] text-xs text-ink"
        type="search"
        aria-label="Search schema"
        placeholder="Find a table…"
        bind:value={query}
        oninput={() => (focusNodeIds = [])}
      />
    </search>
    <nav class="flex gap-[5px]" aria-label="Map options">
      <button
        class={navButton}
        aria-expanded={panel === 'filters'}
        aria-controls="map-options"
        onclick={() => togglePanel('filters')}
        ><Layers3 size={15} />Schemas{#if namespaces.length}<span
            class="text-[10px] text-accent-muted">{namespaces.length}</span
          >{/if}</button
      >
      <button
        class={navButton}
        aria-expanded={panel === 'groups'}
        aria-controls="map-options"
        onclick={() => togglePanel('groups')}><Group size={15} />Groups</button
      >
      <button
        class={navButton}
        aria-label="Source details and actions"
        aria-expanded={panel === 'details'}
        aria-controls="map-options"
        onclick={() => togglePanel('details')}><Ellipsis size={18} /></button
      >
    </nav>
  </header>
  <div class="relative flex min-h-0 flex-1">
    <SvelteFlowProvider
      ><GraphCanvas
        {source}
        {query}
        {namespaces}
        {related}
        {focusNodeIds}
        onselect={(entity) => {
          selected = entity;
          panel = null;
        }}
        {onsave}
      /></SvelteFlowProvider
    >
    {#if panel}
      <aside
        id="map-options"
        class="absolute top-[52px] right-3 z-20 max-h-[calc(100%-68px)] w-[310px] overflow-auto rounded-[10px] border border-line-strong bg-surface-2 shadow-[0_12px_35px_#0007]"
        aria-label={panels[panel].label}
        transition:fly={motion.popover()}
      >
        <header class="flex items-center justify-between border-b border-line px-4 py-3.5">
          <h2 class="text-[13px] font-bold">{panels[panel].title}</h2>
          <button class="icon-btn" aria-label="Close map options" onclick={() => (panel = null)}
            ><X size={16} /></button
          >
        </header>
        {#if panel === 'groups'}
          <CanvasGroups {source} projectId={project.id} {onupdate} onfocus={focusGroup} />
        {:else if panel === 'filters'}
          <SchemaFilter
            graph={source.graph}
            bind:selected={namespaces}
            bind:related
            api={source.kind === 'openapi'}
          />
        {:else}
          <div class="flex flex-col gap-2.5 p-4 text-xs">
            <p class="text-text">
              {project.name} / {source.databaseKind
                ? databaseNames[source.databaseKind]
                : 'OpenAPI'}
            </p>
            <dl class="mt-1.5 mb-3 grid grid-cols-2 gap-2 text-[11px]">
              <dt class="text-muted">Nodes</dt>
              <dd class="text-right">{source.graph.entities.length}</dd>
              <dt class="text-muted">Relationships</dt>
              <dd class="text-right">{source.graph.relations.length}</dd>
              <dt class="text-muted">Last inspected</dt>
              <dd class="text-right">{new Date(source.importedAt).toLocaleString()}</dd>
            </dl>
            {#if source.kind === 'database'}
              <button class="btn justify-start" disabled={refreshing} onclick={onrefresh}
                ><RefreshCw size={14} />{refreshing ? 'Refreshing…' : 'Refresh schema'}</button
              >
              <button class="btn justify-start" onclick={onreconnect}
                ><PlugZap size={14} />Reconnect database</button
              >
            {:else}<button class="btn justify-start" onclick={onapi}
                ><PlugZap size={14} />API connection</button
              >{/if}
            <button class="btn justify-start" onclick={onexport}
              ><Download size={14} />Export</button
            >
            <button class="btn justify-start" onclick={onremove}
              ><Trash2 size={14} />Delete source</button
            >
            {#if source.graph.warnings.length}<details>
                <summary>Import notes ({source.graph.warnings.length})</summary>
                <ul class="list-disc pl-4">
                  {#each source.graph.warnings as warning}<li class="my-2">{warning}</li>{/each}
                </ul>
              </details>{/if}
          </div>
        {/if}
      </aside>
    {/if}
    {#if selected}<Inspector
        entity={selected}
        {source}
        onclose={() => (selected = null)}
        onselect={(entity) => (selected = entity)}
      />{/if}
  </div>
</div>
