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
  const maxFields = 9;
  const store = useStore();
  const detailed = $derived(store.viewport.zoom >= 0.25 || selected);
</script>

<NodeToolbar isVisible={selected} position={Position.Top}>
  <button
    type="button"
    class="node-inspect nodrag nopan"
    aria-label={`Inspect ${data.entity.namespace}.${data.entity.name}`}
    onclick={(event) => {
      event.stopPropagation();
      data.oninspect();
    }}
  >
    <Eye size={16} /><span>Inspect</span>
  </button>
</NodeToolbar>
<article
  class="entity-node"
  class:overview={!detailed}
  class:selected
  class:api-node={data.entity.kind === 'schema'}
  class:operation-node={data.entity.kind === 'operation'}
>
  <Handle type="target" position={Position.Left} id="entity-in" class="entity-port" />
  <Handle type="source" position={Position.Right} id="entity-out" class="entity-port" />
  <div class="entity-heading">
    <div class="entity-title">
      {#if data.entity.method}<span class="method" data-method={data.entity.method}
          >{data.entity.method}</span
        >{:else if data.entity.kind === 'schema'}<Braces
          size={17}
        />{:else if data.entity.kind === 'view'}<Eye size={17} />{:else}<Table2
          size={17}
        />{/if}<strong>{data.entity.name}</strong>
    </div>
    <span class="entity-namespace"
      >{data.entity.namespace}<span
        >{data.entity.kind === 'operation' ? 'ENDPOINT' : data.entity.kind.toUpperCase()}</span
      ></span
    >
  </div>
  <div class="entity-fields">
    {#each data.entity.fields.slice(0, maxFields) as field (field.name)}<div class="entity-field">
        {#if data.incomingFields.has(field.name)}<Handle
            type="target"
            position={Position.Left}
            id={`in-${field.name}`}
            class="field-port"
          />{/if}{#if detailed}<span
            class="field-symbol"
            class:key={field.primaryKey}
            class:foreign={data.foreignFields.has(field.name)}
            >{#if field.primaryKey}<KeyRound
                size={12}
              />{:else if data.foreignFields.has(field.name)}<Link2 size={12} />{:else}<span
                class="field-dot"
              ></span>{/if}</span
          ><span class="field-name">{field.name}</span><span class="field-type"
            >{field.dataType || 'any'}</span
          >{/if}{#if data.foreignFields.has(field.name)}<Handle
            type="source"
            position={Position.Right}
            id={`out-${field.name}`}
            class="field-port"
          />{/if}
      </div>{/each}
  </div>
  {#if data.entity.fields.length > maxFields}<div class="more-fields">
      + {data.entity.fields.length - maxFields} more · inspect for details
    </div>{/if}
  <div class="entity-footer">
    <span
      >{data.entity.fields.length}
      {data.entity.kind === 'table' || data.entity.kind === 'view' ? 'columns' : 'fields'}</span
    >{#if data.entity.fields.some((f) => f.primaryKey)}<span
        ><KeyRound size={10} /> primary key</span
      >{/if}
  </div>
</article>

<style>
  .overview .entity-field {
    height: 29px;
  }
  .overview .entity-fields {
    background: repeating-linear-gradient(transparent 0 28px, #75838d12 28px 29px);
  }
  .node-inspect {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 12px;
    border: 1px solid #3b4540;
    border-radius: 8px;
    background: #141b17;
    color: #d8ecdd;
    font-size: 12px;
    cursor: pointer;
    box-shadow: 0 4px 12px #0005;
  }
  .node-inspect:hover {
    background: #1e2a22;
  }
  .node-inspect:focus-visible {
    outline: 2px solid #9ccd9c;
    outline-offset: 3px;
  }
</style>
