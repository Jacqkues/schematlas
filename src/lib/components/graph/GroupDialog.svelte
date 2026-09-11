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
    <label class="form-label" for="group-name">Group name</label>
    <input
      id="group-name"
      class="mb-[21px] field"
      bind:value={name}
      required
      maxlength="80"
      placeholder="e.g. Customer domain"
    />
    <label class="form-label" for="group-color">Group color</label>
    <div class="mt-2.5 mb-6 flex items-center gap-2.5">
      {#each palette as [label, value]}
        <button
          type="button"
          class="size-[25px] rounded-full border-[3px] border-surface p-0 outline outline-[#363d44] transition-[outline-color] aria-pressed:outline-2 aria-pressed:outline-[#dde3e8]"
          style:background={value}
          aria-label={`${label} group color`}
          aria-pressed={color === value}
          onclick={() => (color = value)}
        ></button>
      {/each}
      <input
        id="group-color"
        type="color"
        class="ml-1.5 h-8 w-[34px] cursor-pointer p-0.5"
        bind:value={color}
        aria-label="Custom group color"
      />
      <span class="font-mono text-[11px] text-muted">{color.toUpperCase()}</span>
    </div>
    <fieldset class="min-w-0 rounded-lg border border-line-strong p-3.5">
      <legend class="px-1.5 text-xs text-[#c9d0d6]"
        >Members <span class="ml-2 text-[#8f98a1]">{members.length} selected</span></legend
      >
      <input
        type="search"
        class="field border-line-strong py-2.5 text-[#d0d6dd]"
        aria-label="Find group members"
        bind:value={query}
        placeholder="Find a table or endpoint…"
      />
      <div class="mt-2 max-h-60 overflow-y-auto">
        {#each entities as entity (entity.id)}
          <label
            class="flex cursor-pointer items-center gap-3 border-b border-line px-1 py-2 last:border-b-0"
            ><input
              type="checkbox"
              class="m-0 size-[15px] flex-none"
              value={entity.id}
              bind:group={members}
            />
            <span class="text-xs [overflow-wrap:anywhere] text-[#d0d6dd]"
              >{entity.name}<small class="mt-[3px] block font-mono text-[10px] text-[#8c96a0]"
                >{entity.namespace}</small
              ></span
            >
          </label>
        {:else}<p class="form-hint">No matching nodes.</p>{/each}
      </div>
    </fieldset>
    {#if error}<p class="form-error" role="alert">{error}</p>{/if}
    <div class="modal-footer">
      <span class="form-hint">Drag the group title to move its nodes together.</span>
      <button type="submit" class="btn btn-primary" disabled={busy || !members.length}>
        {busy ? 'Saving…' : group ? 'Save group' : 'Create group'}
      </button>
    </div>
  </form>
</Modal>
