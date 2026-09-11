<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from '../dialogs/Modal.svelte';
  import { api } from '$lib/services/workspace';
  import type { CanvasGroup, Project, Source } from '$lib/types';
  let {
    source,
    projectId,
    group,
    onclose,
    onupdate,
  }: {
    source: Source;
    projectId: string;
    group?: CanvasGroup;
    onclose: () => void;
    onupdate: (project: Project) => void;
  } = $props();
  let name = $state(untrack(() => group?.name ?? ''));
  let color = $state(untrack(() => group?.color ?? '#879b91'));
  let members = $state<string[]>(untrack(() => [...(group?.nodeIds ?? [])]));
  let query = $state('');
  let busy = $state(false);
  let error = $state('');
  const palette = [
    ['Slate', '#879b91'],
    ['Blue', '#6d9de3'],
    ['Violet', '#ac8cda'],
    ['Rose', '#d48b9e'],
    ['Amber', '#c9a369'],
    ['Teal', '#6eaaa9'],
  ];
  let entities = $derived(
    source.graph.entities.filter((e) =>
      `${e.namespace} ${e.name}`.toLowerCase().includes(query.trim().toLowerCase()),
    ),
  );
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = '';
    try {
      onupdate(
        await api.setGroup(projectId, source.id, { id: group?.id, name, color, nodeIds: members }),
      );
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title={group ? 'Edit group' : 'Create group'}
  subtitle="Choose a color and the nodes that belong together."
  {onclose}
  {busy}
>
  <form onsubmit={submit}>
    <label for="group-name">Group name</label>
    <input
      id="group-name"
      bind:value={name}
      required
      maxlength="80"
      placeholder="e.g. Customer domain"
    />
    <label for="group-color">Group color</label>
    <div class="group-colors">
      {#each palette as [label, value]}
        <button
          type="button"
          class="group-color-swatch"
          style:--swatch={value}
          aria-label={`${label} group color`}
          aria-pressed={color === value}
          onclick={() => (color = value)}
        ></button>
      {/each}
      <input id="group-color" type="color" bind:value={color} aria-label="Custom group color" />
      <span>{color.toUpperCase()}</span>
    </div>
    <fieldset class="group-members">
      <legend>Members <span>{members.length} selected</span></legend>
      <input
        type="search"
        aria-label="Find group members"
        bind:value={query}
        placeholder="Find a table or endpoint…"
      />
      <div class="group-member-list">
        {#each entities as entity (entity.id)}
          <label
            ><input type="checkbox" value={entity.id} bind:group={members} />
            <span>{entity.name}<small>{entity.namespace}</small></span>
          </label>
        {:else}<p class="form-hint">No matching nodes.</p>{/each}
      </div>
    </fieldset>
    {#if error}<p class="form-error" role="alert">{error}</p>{/if}
    <div class="modal-footer">
      <span class="form-hint">Drag the group title to move its nodes together.</span>
      <button type="submit" class="button primary" disabled={busy || !members.length}>
        {busy ? 'Saving…' : group ? 'Save group' : 'Create group'}
      </button>
    </div>
  </form>
</Modal>
