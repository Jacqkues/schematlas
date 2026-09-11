<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import type { AgentSnapshot } from '$lib/services/agent';
  import { Plus, Braces, Database, X, AlertCircle, CheckCircle2 } from '@lucide/svelte';
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
  import type { Project, Position } from '$lib/types';
  import { save } from '@tauri-apps/plugin-dialog';
  import '../app.css';
  import '../dark.css';
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
  onMount(() => {
    void workspace.load();
    let offCanvas: (() => void) | undefined;
    let canvasDisposed = false;
    if (desktop)
      void listen<Project>('workspace:update', ({ payload }) => {
        if (!canvasDisposed)
          workspace.projects = workspace.projects.map((p) => (p.id === payload.id ? payload : p));
      }).then((fn) => {
        if (canvasDisposed) fn();
        else offCanvas = fn;
      });
    if (!desktop) return;
    let disposed = false;
    let unsubscribe: (() => void) | undefined;
    void listen<AgentSnapshot>('agent:update', ({ payload }) => {
      if (disposed || !payload.reviews.length) return;
      if (payload.projectId === workspace.project?.id) agentOpen = true;
      else
        workspace.notice = `Agent waiting for review in ${workspace.projects.find((p) => p.id === payload.projectId)?.name ?? 'another project'}. Open that project’s Local agent panel.`;
    }).then((fn) => {
      if (disposed) fn();
      else unsubscribe = fn;
    });
    return () => {
      disposed = true;
      unsubscribe?.();
      canvasDisposed = true;
      offCanvas?.();
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
      const updated = await api.savePositions(projectId, sourceId, value);
      workspace.projects = workspace.projects.map((p) => (p.id === projectId ? updated : p));
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
{#if desktop}<div class="window-drag-strip" data-tauri-drag-region></div>{/if}
<div class="app-shell" class:native-shell={desktop}>
  <Sidebar
    state={workspace}
    onagent={() => (agentOpen = !agentOpen)}
    oncreate={() => (modal = 'create')}
    onconnect={() => (modal = 'connect')}
    onimport={() => (modal = 'import')}
    onedit={() => (modal = 'edit')}
    onhome={() => (workspace.sourceId = null)}
  />
  <main>
    {#if !desktop}<div class="preview-banner">
        Browser preview <span>Database connections and OpenAPI imports run in the desktop app.</span
        >
      </div>{/if}
    {#if workspace.loading}<div class="loading-state">
        <span class="loader"></span>
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
      <div class="welcome-toolbar">
        <span>{workspace.project ? 'PROJECT OVERVIEW' : 'YOUR WORKSPACE'}</span
        >{#if workspace.project}<div>
            <button class="button" onclick={() => (modal = 'import')}
              ><Braces size={15} /> Import OpenAPI</button
            ><button class="button primary" onclick={() => (modal = 'connect')}
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
{#if workspace.error}<div class="toast error-toast" role="alert">
    <AlertCircle size={18} />
    <p>{workspace.error}</p>
    <button class="icon-button" aria-label="Dismiss error" onclick={() => (workspace.error = '')}
      ><X size={17} /></button
    >
  </div>{:else if workspace.notice}<div class="toast" role="status">
    <CheckCircle2 size={18} />
    <p>{workspace.notice}</p>
    <button
      class="icon-button"
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
