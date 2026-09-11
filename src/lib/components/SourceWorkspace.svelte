<script lang="ts">
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
  let query = $state('');
  let namespaces = $state<string[]>([]);
  let related = $state(true);
  let selected = $state<Entity | null>(null);
  let panel = $state<'groups' | 'filters' | 'details' | null>(null);
  let focusNodeIds = $state<string[]>([]);
  function togglePanel(next: 'groups' | 'filters' | 'details') {
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

<div class="source-workspace graph-first">
  <header class="map-toolbar">
    <div class="map-title">
      {#if source.kind === 'database'}<Database size={17} />{:else}<Braces size={17} />{/if}
      <h1>{source.name}</h1>
    </div>
    <search class="map-search">
      <Search size={14} /><input
        type="search"
        aria-label="Search schema"
        placeholder="Find a table…"
        bind:value={query}
        oninput={() => (focusNodeIds = [])}
      />
    </search>
    <nav aria-label="Map options">
      <button
        class:active={panel === 'filters'}
        aria-expanded={panel === 'filters'}
        aria-controls="map-options"
        onclick={() => togglePanel('filters')}
        ><Layers3 size={15} />Schemas{#if namespaces.length}<span class="filter-count"
            >{namespaces.length}</span
          >{/if}</button
      >
      <button
        class:active={panel === 'groups'}
        aria-expanded={panel === 'groups'}
        aria-controls="map-options"
        onclick={() => togglePanel('groups')}><Group size={15} />Groups</button
      >
      <button
        class="map-more"
        class:active={panel === 'details'}
        aria-label="Source details and actions"
        aria-expanded={panel === 'details'}
        aria-controls="map-options"
        onclick={() => togglePanel('details')}><Ellipsis size={18} /></button
      >
    </nav>
  </header>
  <div class="graph-area">
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
        class="map-options"
        aria-label={panel === 'groups'
          ? 'Manage groups'
          : panel === 'filters'
            ? 'Schema filters'
            : 'Source details'}
      >
        <header>
          <h2>
            {panel === 'groups' ? 'Groups' : panel === 'filters' ? 'Schemas' : 'Source details'}
          </h2>
          <button class="icon-button" aria-label="Close map options" onclick={() => (panel = null)}
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
          <div class="source-detail-body">
            <p>
              {project.name} / {source.databaseKind
                ? databaseNames[source.databaseKind]
                : 'OpenAPI'}
            </p>
            <dl>
              <dt>Nodes</dt>
              <dd>{source.graph.entities.length}</dd>
              <dt>Relationships</dt>
              <dd>{source.graph.relations.length}</dd>
              <dt>Last inspected</dt>
              <dd>{new Date(source.importedAt).toLocaleString()}</dd>
            </dl>
            {#if source.kind === 'database'}
              <button class="button" disabled={refreshing} onclick={onrefresh}
                ><RefreshCw size={14} />{refreshing ? 'Refreshing…' : 'Refresh schema'}</button
              >
              <button class="button" onclick={onreconnect}
                ><PlugZap size={14} />Reconnect database</button
              >
            {:else}<button class="button" onclick={onapi}
                ><PlugZap size={14} />API connection</button
              >{/if}
            <button class="button" onclick={onexport}><Download size={14} />Export</button>
            <button class="button" onclick={onremove}><Trash2 size={14} />Delete source</button>
            {#if source.graph.warnings.length}<details>
                <summary>Import notes ({source.graph.warnings.length})</summary>
                <ul>
                  {#each source.graph.warnings as warning}<li>{warning}</li>{/each}
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

<style>
  .map-toolbar {
    display: flex;
    align-items: center;
    gap: 18px;
    min-height: 58px;
    padding: 0 18px;
    border-bottom: 1px solid #202528;
    background: #0c0f11;
    flex-shrink: 0;
  }
  .map-title {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
    flex: 1;
    color: #a9b7ae;
  }
  h1 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #e0e6e2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .map-search {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #85918a;
    width: clamp(140px, 20vw, 260px);
  }
  .map-search input {
    width: 100%;
    border: 0;
    background: transparent;
    color: #dfe5e1;
    padding: 9px 0;
    font-size: 12px;
    outline-offset: 3px;
  }
  nav {
    display: flex;
    gap: 5px;
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 9px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: transparent;
    color: #aab5ae;
    font-size: 11px;
    cursor: pointer;
  }
  nav button:hover,
  nav button.active {
    background: #1b2420;
    border-color: #354239;
    color: #e0ebe3;
  }
  .filter-count {
    font-size: 10px;
    color: #b4d6be;
  }
  .graph-area {
    position: relative;
  }
  .map-options {
    position: absolute;
    z-index: 20;
    right: 12px;
    top: 52px;
    width: 310px;
    max-height: calc(100% - 68px);
    overflow: auto;
    border: 1px solid #303a34;
    border-radius: 10px;
    background: #101613;
    box-shadow: 0 12px 35px #0007;
  }
  .map-options > header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px;
    border-bottom: 1px solid #27312b;
  }
  h2 {
    font-size: 13px;
    margin: 0;
  }
  .map-options :global(.schema-filter) {
    flex-wrap: wrap;
    padding: 16px;
    border: 0;
    gap: 14px;
  }
  .map-options :global(.filter-label) {
    display: none;
  }
  .map-options :global(.schema-chips) {
    flex-wrap: wrap;
  }
  .map-options :global(.related-toggle) {
    margin: 0;
  }
  .source-detail-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    font-size: 12px;
  }
  .source-detail-body p {
    margin: 0;
    color: #b4c0b8;
  }
  dl {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin: 6px 0 12px;
    font-size: 11px;
  }
  dt {
    color: #87958c;
  }
  dd {
    margin: 0;
    text-align: right;
  }
  .source-detail-body .button {
    justify-content: flex-start;
  }
  .source-detail-body li {
    margin: 8px 0;
  }
  @media (max-width: 1100px) {
    .map-toolbar {
      gap: 8px;
      padding: 0 12px;
    }
    .map-search {
      width: 160px;
    }
  }
</style>
