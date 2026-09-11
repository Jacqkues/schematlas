<script lang="ts">
  import { onMount } from 'svelte';
  import { initializeAppearance } from '$lib/state/appearance.svelte';
  import { fly } from 'svelte/transition';
  import { listen } from '@tauri-apps/api/event';
  import type { AgentSnapshot } from '$lib/services/agent';
  import { Plus, Braces, X, AlertCircle, CheckCircle2 } from '@lucide/svelte';
  import ResizablePanel from '$lib/components/agent/ResizablePanel.svelte';
  import AgentPanel from '$lib/components/agent/AgentPanel.svelte';
  import ApiDialog from '$lib/components/dialogs/ApiDialog.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import SourceWorkspace from '$lib/components/SourceWorkspace.svelte';
  import ProjectDialog from '$lib/components/dialogs/ProjectDialog.svelte';
  import ConnectDialog from '$lib/components/dialogs/ConnectDialog.svelte';
  import ImportDialog from '$lib/components/dialogs/ImportDialog.svelte';
  import ConfirmDialog from '$lib/components/dialogs/ConfirmDialog.svelte';
  import { WorkspaceState } from '$lib/state/workspace.svelte';
  import { api, desktop } from '$lib/services/workspace';
  import { motion } from '$lib/motion';
  import type { Project, Position } from '$lib/types';
  import { save } from '@tauri-apps/plugin-dialog';
  import '../app.css';
  onMount(initializeAppearance);
  const workspace = new WorkspaceState();
  let modal = $state<
    | 'api'
    | 'create'
    | 'edit'
    | 'connect'
    | 'reconnect'
    | 'import'
    | 'delete-project'
    | 'delete-source'
    | null
  >(null);
  let agentOpen = $state(false);
  let refreshing = $state(false);
  let demoBusy = $state(false);
  const toast =
    'fixed bottom-[50px] left-1/2 z-[1000] flex w-max max-w-[640px] -translate-x-1/2 items-center gap-3 rounded-lg border border-line bg-surface py-3.5 pr-[15px] pl-[18px] text-soft shadow-[0_7px_30px_#0006]';
  onMount(() => {
    void workspace.load();
    if (!desktop) return;
    let disposed = false;
    const stops: (() => void)[] = [];
    const subscribe = <T,>(event: string, handler: (payload: T) => void) =>
      void listen<T>(event, ({ payload }) => {
        if (!disposed) handler(payload);
      }).then((stop) => (disposed ? stop() : stops.push(stop)));
    subscribe<Project>('workspace:update', (project) => workspace.replace(project));
    subscribe<AgentSnapshot>('agent:update', (snapshot) => {
      if (!snapshot.reviews.length) return;
      if (snapshot.projectId === workspace.project?.id) agentOpen = true;
      else
        workspace.notice = `Agent waiting for review in ${workspace.projects.find((p) => p.id === snapshot.projectId)?.name ?? 'another project'}. Open that project’s Local agent panel.`;
    });
    return () => {
      disposed = true;
      for (const stop of stops) stop();
    };
  });
  async function demo() {
    demoBusy = true;
    try {
      workspace.upsert(await api.demo(), false);
      workspace.sourceId = workspace.project?.sources[0]?.id ?? null;
    } catch (e) {
      workspace.fail(e);
    } finally {
      demoBusy = false;
    }
  }
  async function refresh() {
    if (!workspace.project || !workspace.source) return;
    refreshing = true;
    try {
      workspace.upsert(await api.refresh(workspace.project.id, workspace.source.id));
      workspace.notice = 'Schema refreshed.';
    } catch (e) {
      workspace.fail(e);
      if (String(e).includes('Reconnect')) modal = 'reconnect';
    } finally {
      refreshing = false;
    }
  }
  async function positions(projectId: string, sourceId: string, value: Record<string, Position>) {
    try {
      workspace.replace(await api.savePositions(projectId, sourceId, value));
    } catch (e) {
      workspace.fail(e);
    }
  }
  async function exportSource() {
    if (!workspace.project || !workspace.source) return;
    try {
      if (desktop) {
        const path = await save({
          title: 'Export schema map',
          defaultPath: `${workspace.source.name.replace(/[^a-zA-Z0-9_-]/g, '-')}.json`,
          filters: [{ name: 'JSON', extensions: ['json'] }],
        });
        if (path) {
          await api.export(workspace.project.id, workspace.source.id, path);
          workspace.notice = 'Schema map exported.';
        }
      } else {
        const blob = new Blob([JSON.stringify(workspace.source, null, 2)], {
          type: 'application/json',
        });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'schema-map.json';
        a.click();
        setTimeout(() => URL.revokeObjectURL(url), 1000);
      }
    } catch (e) {
      workspace.fail(e);
    }
  }
</script>

<svelte:head
  ><title>Schematlas — Your data, mapped.</title><meta
    name="description"
    content="A local workspace to explore database schemas and OpenAPI definitions."
  /></svelte:head
>
{#if desktop}<div
    class="fixed inset-x-0 top-0 z-[100] h-[30px] bg-sidebar"
    data-tauri-drag-region
  ></div>{/if}
<div
  class={[
    'flex h-dvh min-h-[620px] overflow-hidden bg-bg',
    desktop && 'relative mt-[30px] h-[calc(100dvh-30px)]',
  ]}
>
  <Sidebar
    state={workspace}
    onagent={() => (agentOpen = !agentOpen)}
    oncreate={() => (modal = 'create')}
    onconnect={() => (modal = 'connect')}
    onimport={() => (modal = 'import')}
    onedit={() => (modal = 'edit')}
    onhome={() => (workspace.sourceId = null)}
  />
  <main class="flex min-w-0 flex-1 flex-col">
    {#if !desktop}<div
        class="shrink-0 border-b border-accent-line bg-accent-soft p-1.5 text-center text-[10px] font-semibold text-accent-text"
      >
        Browser preview <span class="ml-2.5 font-normal text-faint"
          >Database connections and OpenAPI imports run in the desktop app.</span
        >
      </div>{/if}
    {#if workspace.loading}<div
        class="flex flex-1 flex-col items-center justify-center gap-[18px] text-[13px] text-muted"
      >
        <span
          class="size-[25px] animate-spin rounded-full border-2 border-line-strong border-t-accent"
        ></span>
        <p>Opening your workspace…</p>
      </div>{:else if workspace.project && workspace.source}
      {#key `${workspace.project.id}:${workspace.source.id}`}<SourceWorkspace
          project={workspace.project}
          source={workspace.source}
          onupdate={(p) => workspace.upsert(p)}
          onapi={() => (modal = 'api')}
          onrefresh={refresh}
          onreconnect={() => (modal = 'reconnect')}
          onexport={exportSource}
          onremove={() => (modal = 'delete-source')}
          onsave={(value) => {
            if (workspace.project && workspace.source)
              void positions(workspace.project.id, workspace.source.id, value);
          }}
          {refreshing}
        />{/key}
    {:else}
      <div
        class="flex h-20 items-center justify-between border-b border-line px-10 max-[1000px]:px-[30px]"
      >
        <span class="text-[10px] tracking-[1.6px] text-muted"
          >{workspace.project ? 'PROJECT OVERVIEW' : 'YOUR WORKSPACE'}</span
        >{#if workspace.project}<div class="flex gap-2.5">
            <button class="btn" onclick={() => (modal = 'import')}
              ><Braces size={15} /> Import OpenAPI</button
            ><button class="btn btn-primary" onclick={() => (modal = 'connect')}
              ><Plus size={15} /> Connect database</button
            >
          </div>{/if}
      </div>
      <EmptyState
        hasProject={!!workspace.project}
        name={workspace.project?.name}
        oncreate={() => (modal = 'create')}
        onconnect={() => (modal = 'connect')}
        onimport={() => (modal = 'import')}
        ondemo={demo}
        busy={demoBusy}
      />
    {/if}
  </main>
  {#if agentOpen && workspace.project}{#key workspace.project.id}<ResizablePanel
        ><AgentPanel
          projectId={workspace.project.id}
          projectName={workspace.project.name}
          onclose={() => (agentOpen = false)}
        /></ResizablePanel
      >{/key}{/if}
</div>
{#if workspace.error}<div
    class={[toast, 'border-danger-line text-text']}
    role="alert"
    transition:fly={motion.toast()}
  >
    <AlertCircle size={18} class="shrink-0" />
    <p class="max-w-[540px] text-xs leading-relaxed [overflow-wrap:anywhere]">{workspace.error}</p>
    <button class="icon-btn" aria-label="Dismiss error" onclick={() => (workspace.error = '')}
      ><X size={17} /></button
    >
  </div>{:else if workspace.notice}<div class={toast} role="status" transition:fly={motion.toast()}>
    <CheckCircle2 size={18} class="shrink-0" />
    <p class="max-w-[540px] text-xs leading-relaxed [overflow-wrap:anywhere]">{workspace.notice}</p>
    <button
      class="icon-btn"
      aria-label="Dismiss notification"
      onclick={() => (workspace.notice = '')}><X size={17} /></button
    >
  </div>{/if}
{#if modal === 'create' || modal === 'edit'}<ProjectDialog
    project={modal === 'edit' ? (workspace.project ?? undefined) : undefined}
    onclose={() => (modal = null)}
    onsave={(p) => workspace.upsert(p)}
    ondelete={() => (modal = 'delete-project')}
  />
{:else if (modal === 'connect' || modal === 'reconnect') && workspace.project}<ConnectDialog
    projectId={workspace.project.id}
    source={modal === 'reconnect' ? (workspace.source ?? undefined) : undefined}
    onclose={() => (modal = null)}
    onsave={(p) => workspace.upsert(p, modal === 'connect')}
  />
{:else if modal === 'import' && workspace.project}<ImportDialog
    projectId={workspace.project.id}
    onclose={() => (modal = null)}
    onsave={(p) => workspace.upsert(p, true)}
  />
{:else if modal === 'delete-project' && workspace.project}<ConfirmDialog
    title="Delete this project?"
    message={`Delete “${workspace.project.name}” and its saved maps from this workspace? Your databases and original files are unaffected.`}
    onclose={() => (modal = null)}
    onconfirm={() => workspace.deleteProject()}
  />
{:else if modal === 'delete-source' && workspace.project && workspace.source}<ConfirmDialog
    title="Delete this source?"
    message={`Remove “${workspace.source.name}” and its saved map from this project? Your database or original file is unaffected.`}
    onclose={() => (modal = null)}
    onconfirm={async () => {
      if (workspace.project && workspace.source)
        workspace.upsert(await api.removeSource(workspace.project.id, workspace.source.id));
    }}
  />{/if}

{#if modal === 'api' && workspace.project && workspace.source}<ApiDialog
    projectId={workspace.project.id}
    source={workspace.source}
    onclose={() => (modal = null)}
    onsave={(p) => workspace.upsert(p)}
  />{/if}
