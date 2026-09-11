<script lang="ts">
  import { Link2 } from '@lucide/svelte';
  import type { Graph } from '$lib/types';
  import { schemaNamespaces } from '$lib/services/layout';
  let {
    graph,
    selected = $bindable<string[]>([]),
    related = $bindable(true),
    api = false,
  }: { graph: Graph; selected?: string[]; related?: boolean; api?: boolean } = $props();
  let namespaces = $derived(schemaNamespaces(graph));
  const chip =
    'flex items-center gap-[9px] rounded-[5px] border border-line-soft bg-surface px-[9px] py-[5px] font-mono text-[10px] whitespace-nowrap text-soft transition-colors aria-pressed:border-line-strong aria-pressed:bg-surface-3 aria-pressed:text-ink';
  function toggle(name: string) {
    selected = selected.includes(name) ? selected.filter((s) => s !== name) : [...selected, name];
  }
</script>

<div
  class="flex flex-wrap items-center gap-3.5 p-4"
  aria-label={api ? 'Filter API groups' : 'Filter database schemas'}
>
  <div class="flex flex-1 flex-wrap gap-1.5 p-0.5">
    <button class={chip} aria-pressed={!selected.length} onclick={() => (selected = [])}
      >All <span class="text-[8px] text-soft">{namespaces.length}</span></button
    >
    {#each namespaces as namespace}
      <button
        class={chip}
        aria-pressed={selected.includes(namespace.name)}
        onclick={() => toggle(namespace.name)}
        >{namespace.name}<span class="text-[8px] text-soft">{namespace.count}</span></button
      >
    {/each}
  </div>
  <label class="flex items-center gap-1.5 text-[10px] whitespace-nowrap text-soft"
    ><input type="checkbox" class="accent-accent" bind:checked={related} /><Link2 size={13} /> Include
    linked {api ? 'groups' : 'schemas'}</label
  >
</div>
