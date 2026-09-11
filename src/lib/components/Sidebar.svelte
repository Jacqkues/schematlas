<script lang="ts">
  import {
    Plus,
    FolderOpen,
    Database,
    Braces,
    ChevronDown,
    Layers3,
    ArrowUpRight,
    Pencil,
    HardDrive,
  } from '@lucide/svelte';
  import Brand from './Brand.svelte';
  import type { WorkspaceState } from '$lib/state/workspace.svelte';
  let {
    state,
    onagent,
    oncreate,
    onconnect,
    onimport,
    onedit,
    onhome,
  }: {
    state: WorkspaceState;
    onagent: () => void;
    oncreate: () => void;
    onconnect: () => void;
    onimport: () => void;
    onedit: () => void;
    onhome: () => void;
  } = $props();
</script>

<aside class="sidebar">
  <button class="brand-button" onclick={onhome} aria-label="Schematlas projects"><Brand /></button
  >
  <div class="workspace-label">LOCAL WORKSPACE <span>01</span></div>
  <div class="project-picker">
    <FolderOpen size={16} /><select
      aria-label="Current project"
      value={state.projectId ?? ''}
      onchange={(e) => state.selectProject(e.currentTarget.value || null)}
      ><option value="" disabled>Select a project</option>{#each state.projects as p}<option
          value={p.id}>{p.name}</option
        >{/each}</select
    ><ChevronDown size={14} />
  </div>
  <button class="sidebar-action" onclick={oncreate}
    ><Plus size={15} /> New project <kbd>＋</kbd></button
  >
  <div class="side-divider"></div>
  <div class="section-heading">
    EXPLORER {#if state.project}<button
        class="icon-button"
        aria-label="Edit project"
        onclick={onedit}><Pencil size={13} /></button
      >{/if}
  </div>
  {#if state.project}
    <div class="source-heading">
      <span><Database size={13} /> DATABASES</span><button
        class="icon-button"
        aria-label="Connect database"
        onclick={onconnect}><Plus size={14} /></button
      >
    </div>
    {#each state.project.sources.filter((s) => s.kind === 'database') as source}
      <button
        class:active={state.sourceId === source.id}
        class="source-link"
        onclick={() => state.selectSource(source)}
        ><Database size={16} /><span>{source.name}</span><small
          >{source.graph.entities.length}</small
        ></button
      >
    {:else}<p class="source-empty">No databases connected</p>{/each}
    <div class="source-heading second">
      <span><Braces size={13} /> API DEFINITIONS</span><button
        class="icon-button"
        aria-label="Import OpenAPI"
        onclick={onimport}><Plus size={14} /></button
      >
    </div>
    {#each state.project.sources.filter((s) => s.kind === 'openapi') as source}
      <button
        class:active={state.sourceId === source.id}
        class="source-link api-link"
        onclick={() => state.selectSource(source)}
        ><Braces size={16} /><span>{source.name}</span><small>{source.graph.entities.length}</small
        ></button
      >
    {:else}<p class="source-empty">No definitions imported</p>{/each}
  {:else}<p class="source-empty intro">
      Create a project to organize your databases and APIs.
    </p>{/if}
  <div class="sidebar-bottom">
    {#if state.project}<button class="button agent-toggle" onclick={onagent}
        ><span>⌘</span> Local agent <small>ACP</small></button
      >{/if}
    <div class="local-card">
      <HardDrive size={17} />
      <div>
        <strong>Made to stay local.</strong>
        <p>Your workspace lives on this Mac.</p>
      </div>
    </div>
    <div class="sidebar-footer">
      <span><span class="status-dot"></span> Local workspace</span><span>v0.3</span>
    </div>
  </div>
</aside>
