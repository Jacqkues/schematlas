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

<div class="group-manager">
  <div class="group-actions">
    <button
      class="button"
      disabled={busy || !source.graph.entities.length}
      onclick={() => (editing = null)}><Plus size={13} />New group</button
    >
    {#if source.layoutBackup}<button
        class="icon-button"
        aria-label="Undo canvas edit"
        disabled={busy}
        onclick={() => change('undo_canvas')}><Undo2 size={15} /></button
      >{/if}
  </div>
  {#each source.groups ?? [] as group}
    <div class="group-row">
      <button
        class="group-focus"
        aria-label={`Focus group ${group.name}`}
        onclick={() => onfocus(group.nodeIds)}
        ><span style:background={group.color ?? '#879b91'}></span><span>{group.name}</span><Scan
          size={13}
        /></button
      >
      <button
        class="icon-button"
        aria-label={`Edit group ${group.name}`}
        disabled={busy}
        onclick={() => (editing = group)}><Pencil size={13} /></button
      >
      <button
        class="icon-button"
        aria-label={`Remove group ${group.name}`}
        disabled={busy}
        onclick={() => change('remove_canvas_group', group.id)}><X size={13} /></button
      >
    </div>
  {:else}<p>No groups yet. Create one to organize related nodes.</p>{/each}
</div>
{#if error}<p class="form-error" role="alert">{error}</p>{/if}

{#if editing !== undefined}<GroupDialog
    {source}
    {projectId}
    group={editing ?? undefined}
    {onupdate}
    onclose={() => (editing = undefined)}
  />{/if}

<style>
  .group-manager {
    padding: 12px;
  }
  .group-actions {
    display: flex;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .group-row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-top: 1px solid #252e28;
    padding: 7px 0;
  }
  .group-focus {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 6px 4px;
    border: 0;
    background: transparent;
    color: #d2ded5;
    text-align: left;
    font-size: 11px;
    cursor: pointer;
  }
  .group-focus > span:first-child {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .group-focus > span:nth-child(2) {
    flex: 1;
  }
  .group-focus:hover {
    color: #fff;
  }
  .group-row .icon-button {
    width: 25px;
    height: 28px;
  }
  p {
    font-size: 12px;
    line-height: 1.7;
    color: #9caaa1;
  }
</style>
