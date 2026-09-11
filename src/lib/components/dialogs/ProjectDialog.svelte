<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { api } from '$lib/services/workspace';
  import type { Project } from '$lib/types';
  import { ArrowRight, Trash2 } from '@lucide/svelte';
  let {
    project,
    onclose,
    onsave,
    ondelete,
  }: {
    project?: Project;
    onclose: () => void;
    onsave: (p: Project) => void;
    ondelete?: () => void;
  } = $props();
  let name = $state(untrack(() => project?.name ?? ''));
  let description = $state(untrack(() => project?.description ?? ''));
  let busy = $state(false);
  let error = $state('');
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const result = project
        ? await api.rename(project.id, name, description)
        : await api.create(name, description);
      onsave(result);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title={project ? 'Project settings' : 'A place for your architecture.'}
  subtitle={project
    ? 'Keep your workspace organized.'
    : 'Give your databases and APIs a shared home.'}
  {onclose}
  {busy}
>
  <form onsubmit={submit}>
    <label class="form-label" for="project-name"
      >Project name <span class="text-accent">*</span></label
    ><input
      id="project-name"
      class="mb-[21px] field"
      name="name"
      bind:value={name}
      placeholder="e.g. Customer platform"
      required
      maxlength="80"
    />
    <label class="form-label" for="project-description"
      >Description <small class="float-right text-[10px] font-normal text-muted">Optional</small
      ></label
    ><textarea
      id="project-description"
      class="mb-[21px] field max-h-60 min-h-[90px] resize-y"
      name="description"
      bind:value={description}
      placeholder="What are you mapping?"
      maxlength="2000"
      rows="3"></textarea>
    {#if error}<p role="alert" class="form-error">{error}</p>{/if}
    <div class="modal-footer">
      {#if project}<button
          class="btn btn-ghost-danger"
          type="button"
          onclick={ondelete}
          disabled={busy}><Trash2 size={15} /> Delete project</button
        >{:else}<span class="form-hint">Saved locally on this computer.</span>{/if}<button
        type="submit"
        class="btn btn-primary"
        disabled={busy}
        >{busy ? 'Saving…' : project ? 'Save changes' : 'Create project'}<ArrowRight
          size={16}
        /></button
      >
    </div>
  </form>
</Modal>
