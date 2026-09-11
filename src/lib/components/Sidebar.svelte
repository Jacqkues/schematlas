<script lang="ts">
  import {
    Plus,
    FolderOpen,
    Database,
    Braces,
    ChevronDown,
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

{#snippet sources(kind: 'database' | 'openapi')}
  {@const api = kind === 'openapi'}
  <div
    class={[
      'mr-1.5 mb-[5px] ml-2.5 flex items-center justify-between text-[9px] font-[650] tracking-[1px] text-muted',
      api && 'mt-[25px]',
    ]}
  >
    <span class="flex items-center gap-[7px]"
      >{#if api}<Braces size={13} />{:else}<Database size={13} />{/if}
      {api ? 'API DEFINITIONS' : 'DATABASES'}</span
    ><button
      class="icon-btn"
      aria-label={api ? 'Import OpenAPI' : 'Connect database'}
      onclick={api ? onimport : onconnect}><Plus size={14} /></button
    >
  </div>
  {#each (state.project?.sources ?? []).filter((s) => s.kind === kind) as source (source.id)}
    <button
      class={[
        'my-[3px] flex w-full items-center gap-2.5 rounded-md border px-3 py-[11px] text-left text-xs transition-colors',
        state.sourceId === source.id
          ? 'border-[#354039] bg-surface-4 text-[#e4e9ed] shadow-[inset_2px_0_var(--color-accent)] [&>svg]:text-sage'
          : 'border-transparent text-text hover:bg-surface',
      ]}
      onclick={() => state.selectSource(source)}
      >{#if api}<Braces size={16} />{:else}<Database size={16} />{/if}<span class="truncate"
        >{source.name}</span
      ><small class="ml-auto font-mono text-[10px] text-muted">{source.graph.entities.length}</small
      ></button
    >
  {:else}<p class="mx-3 my-2.5 text-[11px] leading-relaxed text-muted">
      {api ? 'No definitions imported' : 'No databases connected'}
    </p>{/each}
{/snippet}

<aside
  class="flex w-[250px] shrink-0 flex-col border-r border-line bg-sidebar px-4 pt-7 max-[1180px]:w-[218px] max-[1180px]:px-3"
>
  <button class="px-[9px] text-left" onclick={onhome} aria-label="Schematlas projects"
    ><Brand /></button
  >
  <div
    class="mx-2.5 mt-[37px] mb-3 flex justify-between text-[9px] font-bold tracking-[1.5px] text-muted"
  >
    LOCAL WORKSPACE <span class="font-mono">01</span>
  </div>
  <div
    class="relative flex items-center gap-[9px] rounded-[7px] border border-line-strong bg-surface-3 px-2.5 py-[11px] text-text"
  >
    <FolderOpen size={16} class="shrink-0 text-sage" /><select
      class="w-full min-w-0 cursor-pointer appearance-none border-0 bg-transparent pr-3 text-xs font-semibold text-ink"
      aria-label="Current project"
      value={state.projectId ?? ''}
      onchange={(e) => state.selectProject(e.currentTarget.value || null)}
      ><option value="" disabled>Select a project</option>{#each state.projects as p}<option
          value={p.id}>{p.name}</option
        >{/each}</select
    ><ChevronDown size={14} class="pointer-events-none absolute right-2.5" />
  </div>
  <button
    class="flex w-full items-center gap-2 px-2.5 py-[13px] text-left text-xs text-muted transition-colors hover:text-accent"
    onclick={oncreate}
    ><Plus size={15} /> New project <kbd class="ml-auto text-[13px]">＋</kbd></button
  >
  <div class="mx-2 mt-2.5 mb-[23px] h-px bg-line-soft"></div>
  <div class="mx-2.5 mb-[15px] section-heading">
    EXPLORER {#if state.project}<button class="icon-btn" aria-label="Edit project" onclick={onedit}
        ><Pencil size={13} /></button
      >{/if}
  </div>
  {#if state.project}
    {@render sources('database')}
    {@render sources('openapi')}
  {:else}<p class="mx-2.5 my-0.5 text-xs leading-relaxed text-muted">
      Create a project to organize your databases and APIs.
    </p>{/if}
  <div class="mt-auto pt-[30px]">
    {#if state.project}<button
        class="mb-[18px] btn w-full justify-start px-3 py-[11px]"
        onclick={onagent}
        ><span class="text-[19px] text-sage">⌘</span> Local agent
        <small class="ml-auto font-mono text-[10px] text-soft">ACP</small></button
      >{/if}
    <div class="flex gap-2.5 px-2.5 pt-3.5 pb-[23px] text-soft">
      <HardDrive size={17} />
      <div>
        <strong class="text-[11px] font-medium">Made to stay local.</strong>
        <p class="mt-[3px] text-[10px] leading-relaxed text-muted">
          Your workspace lives on this Mac.
        </p>
      </div>
    </div>
    <div
      class="flex h-11 items-center justify-between border-t border-line-soft text-[9px] text-muted"
    >
      <span class="flex items-center gap-1.5"><span class="status-dot"></span> Local workspace</span
      ><span>v0.3</span>
    </div>
  </div>
</aside>
