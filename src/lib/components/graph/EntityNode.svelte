<script lang="ts">
  import {
    Handle,
    NodeToolbar,
    Position,
    useStore,
    type NodeProps,
    type Node,
  } from '@xyflow/svelte';
  import { Table2, Braces, KeyRound, Link2, Eye } from '@lucide/svelte';
  import { MAX_FIELDS } from '$lib/services/layout';
  import type { Entity } from '$lib/types';
  type EntityFlowNode = Node<
    {
      entity: Entity;
      foreignFields: Set<string>;
      incomingFields: Set<string>;
      oninspect: () => void;
    },
    'entity'
  >;
  let { data, selected }: NodeProps<EntityFlowNode> = $props();
  const store = useStore();
  // At overview zoom, column text is dropped to reduce rendering work; the selected card stays readable.
  const detailed = $derived(store.viewport.zoom >= 0.25 || selected);
  const fieldPort =
    'size-[5px] border border-surface bg-soft opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-data-selected:opacity-100';
</script>

<NodeToolbar isVisible={selected} position={Position.Top}>
  <button
    type="button"
    class="nodrag nopan flex items-center gap-[7px] rounded-lg border border-accent-line bg-accent-soft px-3 py-2 text-xs text-accent-text shadow-[0_4px_12px_#0005] transition-colors hover:bg-[#1e2a22]"
    aria-label={`Inspect ${data.entity.namespace}.${data.entity.name}`}
    onclick={(event) => {
      event.stopPropagation();
      data.oninspect();
    }}
  >
    <Eye size={16} /><span>Inspect</span>
  </button>
</NodeToolbar>
<article class="entity-node group" data-selected={selected || undefined}>
  <Handle
    type="target"
    position={Position.Left}
    id="entity-in"
    class="top-[31px] size-[7px] border-2 border-surface bg-[#9ea0a3]"
  />
  <Handle
    type="source"
    position={Position.Right}
    id="entity-out"
    class="top-[31px] size-[7px] border-2 border-surface bg-[#9ea0a3]"
  />
  <div class="h-16 rounded-t-[7px] border-b border-line-soft bg-[#151a1f] px-3.5 pt-3.5 pb-2.5">
    <div class="entity-title flex items-center gap-2 text-[#d3d9df]">
      {#if data.entity.method}<span
          class={[
            'rounded-[3px] bg-[#131518] px-[5px] py-[3px] font-mono text-[8px] font-bold text-soft',
            data.entity.method === 'DELETE' && 'bg-[#36383b] text-[#edb1ac]',
          ]}>{data.entity.method}</span
        >{:else if data.entity.kind === 'schema'}<Braces
          size={17}
        />{:else if data.entity.kind === 'view'}<Eye size={17} />{:else}<Table2
          size={17}
        />{/if}<strong
        class={[
          'truncate font-mono font-[650] tracking-[-0.25px] text-ink',
          data.entity.kind === 'operation' ? 'text-[11px]' : 'text-sm',
        ]}>{data.entity.name}</strong
      >
    </div>
    <span class="mt-[7px] ml-[25px] flex justify-between font-mono text-[9px] text-faint"
      >{data.entity.namespace}<span class="text-[7px] tracking-[0.9px]"
        >{data.entity.kind === 'operation' ? 'ENDPOINT' : data.entity.kind.toUpperCase()}</span
      ></span
    >
  </div>
  <div
    class={[
      'py-1.5 text-[#bdc5ce] [contain:layout_style]',
      !detailed && 'bg-[repeating-linear-gradient(transparent_0_28px,#75838d12_28px_29px)]',
    ]}
  >
    {#each data.entity.fields.slice(0, MAX_FIELDS) as field (field.name)}
      {@const foreign = data.foreignFields.has(field.name)}
      <div
        class="relative flex h-[29px] items-center gap-[7px] px-3 font-mono text-xs transition-colors hover:bg-surface-3"
      >
        {#if data.incomingFields.has(field.name)}<Handle
            type="target"
            position={Position.Left}
            id={`in-${field.name}`}
            class={fieldPort}
          />{/if}{#if detailed}<span
            class="flex w-[13px] shrink-0 items-center justify-center text-soft"
            >{#if field.primaryKey}<KeyRound size={12} />{:else if foreign}<Link2
                size={12}
              />{:else}<span class="size-[3px] rounded-full bg-[#7c7e81]"></span>{/if}</span
          ><span class="truncate">{field.name}</span><span
            class="ml-auto max-w-[105px] truncate text-[10px] text-faint"
            >{field.dataType || 'any'}</span
          >{/if}{#if foreign}<Handle
            type="source"
            position={Position.Right}
            id={`out-${field.name}`}
            class={fieldPort}
          />{/if}
      </div>
    {/each}
  </div>
  {#if data.entity.fields.length > MAX_FIELDS}<div
      class="h-[29px] bg-[#0e1013] px-[13px] py-[7px] text-[9px] text-[#bec0c3]"
    >
      + {data.entity.fields.length - MAX_FIELDS} more · inspect for details
    </div>{/if}
  <div
    class="flex h-7 justify-between rounded-b-[7px] border-t border-line-soft px-[13px] py-2 font-mono text-[9px] text-faint"
  >
    <span class="flex items-center gap-1"
      >{data.entity.fields.length}
      {data.entity.kind === 'table' || data.entity.kind === 'view' ? 'columns' : 'fields'}</span
    >{#if data.entity.fields.some((f) => f.primaryKey)}<span class="flex items-center gap-1"
        ><KeyRound size={10} /> primary key</span
      >{/if}
  </div>
</article>
