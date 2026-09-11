<script lang="ts">
  import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
  let { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, data }: EdgeProps =
    $props();
  const path = $derived(
    getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })[0],
  );
</script>

<!-- Highlight state arrives as a class on the Svelte Flow edge wrapper; the stroke animates between states. -->
<BaseEdge
  {id}
  {path}
  interactionWidth={12}
  class="stroke-edge [stroke-width:1.3] transition-[stroke,stroke-width,opacity] duration-200 [.edge-active_&]:stroke-edge-active [.edge-active_&]:[stroke-width:2] [.edge-muted_&]:stroke-[#46545f] [.edge-muted_&]:[stroke-width:1] [.edge-muted_&]:opacity-[0.14] [.selected_&]:stroke-accent [.selected_&]:[stroke-width:2]"
/>
{#if data?.showLabels}
  <g class="pointer-events-auto animate-fade-in" aria-label={String(data.description ?? '')}>
    <title>{String(data.description ?? '')}</title>
    {#each [{ x: sourceX + 34, y: sourceY - 12, text: data.sourceCardinality }, { x: targetX - 34, y: targetY - 12, text: data.targetCardinality }] as label}
      {#if label.text}<g transform={`translate(${label.x},${label.y})`}
          ><rect
            class="fill-accent-soft stroke-[#668774] [stroke-width:1]"
            x="-24"
            y="-12"
            width="48"
            height="23"
            rx="5"
          /><text
            class="fill-accent-text font-mono text-xs font-semibold"
            text-anchor="middle"
            dominant-baseline="central">{String(label.text)}</text
          ></g
        >{/if}
    {/each}
  </g>
{/if}
