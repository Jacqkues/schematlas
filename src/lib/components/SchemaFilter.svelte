<script lang="ts">
  import { Layers3, Link2 } from '@lucide/svelte';
  import type { Graph } from '$lib/types';
  import { schemaNamespaces } from '$lib/services/layout';
  let {
    graph,
    selected = $bindable<string[]>([]),
    related = $bindable(true),
    api = false,
  }: { graph: Graph; selected?: string[]; related?: boolean; api?: boolean } = $props();
  let namespaces = $derived(schemaNamespaces(graph));
  function toggle(name: string) {
    selected = selected.includes(name) ? selected.filter((s) => s !== name) : [...selected, name];
  }
</script>

<div class="schema-filter" aria-label={api ? 'Filter API groups' : 'Filter database schemas'}>
  <span class="filter-label"><Layers3 size={14} />{api ? 'GROUPS' : 'SCHEMAS'}</span>
  <div class="schema-chips">
    <button
      class:chosen={!selected.length}
      aria-pressed={!selected.length}
      onclick={() => (selected = [])}>All <span>{namespaces.length}</span></button
    >
    {#each namespaces as namespace}
      <button
        class:chosen={selected.includes(namespace.name)}
        aria-pressed={selected.includes(namespace.name)}
        onclick={() => toggle(namespace.name)}
        >{namespace.name}<span>{namespace.count}</span></button
      >
    {/each}
  </div>
  <label class="related-toggle"
    ><input type="checkbox" bind:checked={related} /><Link2 size={13} /> Include linked {api
      ? 'groups'
      : 'schemas'}</label
  >
</div>
