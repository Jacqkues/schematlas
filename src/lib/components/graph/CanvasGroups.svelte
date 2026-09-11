<script lang="ts">
  import { api } from '$lib/services/workspace';
  import GroupDialog from './GroupDialog.svelte';
  import { Undo2, X, Pencil, Scan, Plus } from '@lucide/svelte';
  import type { CanvasGroup, Source, Project } from '$lib/types';
  let {
    source,
    projectId,
    onupdate,
    onfocus,
  }: {
    source: Source;
    projectId: string;
    onupdate: (p: Project) => void;
    onfocus: (ids: string[]) => void;
  } = $props();
  let editing = $state<CanvasGroup | null | undefined>(undefined);
  let error = $state('');
  let busy = $state(false);
  async function change(command: string, groupId?: string) {
    busy = true;
    error = '';
    try {
      onupdate(
        await (command === 'undo_canvas'
          ? api.undoCanvas(projectId, source.id)
          : api.removeGroup(projectId, source.id, groupId!)),
      );
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="p-3">
  <div class="mb-2.5 flex justify-between">
    <button
      class="btn"
      disabled={busy || !source.graph.entities.length}
      onclick={() => (editing = null)}><Plus size={13} />New group</button
    >
    {#if source.layoutBackup}<button
        class="icon-btn"
        aria-label="Undo canvas edit"
        disabled={busy}
        onclick={() => change('undo_canvas')}><Undo2 size={15} /></button
      >{/if}
  </div>
  {#each source.groups ?? [] as group}
    <div class="flex items-center gap-0.5 border-t border-line py-[7px]">
      <button
        class="flex min-w-0 flex-1 items-center gap-2 px-1 py-1.5 text-left text-[11px] text-text transition-colors hover:text-ink"
        aria-label={`Focus group ${group.name}`}
        onclick={() => onfocus(group.nodeIds)}
        ><span class="size-1.5 shrink-0 rounded-full" style:background={group.color ?? '#879b91'}
        ></span><span class="flex-1">{group.name}</span><Scan size={13} /></button
      >
      <button
        class="icon-btn h-7 w-[25px]"
        aria-label={`Edit group ${group.name}`}
        disabled={busy}
        onclick={() => (editing = group)}><Pencil size={13} /></button
      >
      <button
        class="icon-btn h-7 w-[25px]"
        aria-label={`Remove group ${group.name}`}
        disabled={busy}
        onclick={() => change('remove_canvas_group', group.id)}><X size={13} /></button
      >
    </div>
  {:else}<p class="text-xs leading-relaxed text-muted">
      No groups yet. Create one to organize related nodes.
    </p>{/each}
</div>
{#if error}<p class="mx-3 form-error mb-3" role="alert">{error}</p>{/if}

{#if editing !== undefined}<GroupDialog
    {source}
    {projectId}
    group={editing ?? undefined}
    {onupdate}
    onclose={() => (editing = undefined)}
  />{/if}
