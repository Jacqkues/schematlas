<script lang="ts">
  import { BaseEdge, getBezierPath, type EdgeProps } from '@xyflow/svelte';
  let {
    id,
    sourceX,
    sourceY,
    targetX,
    targetY,
    sourcePosition,
    targetPosition,
    style,
    data,
  }: EdgeProps = $props();
  const path = $derived(
    getBezierPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition })[0],
  );
</script>

<BaseEdge {id} {path} {style} interactionWidth={12} />
{#if data?.showLabels}
  <g class="cardinality" aria-label={String(data.description ?? '')}>
    <title>{String(data.description ?? '')}</title>
    {#each [{ x: sourceX + 34, y: sourceY - 12, text: data.sourceCardinality }, { x: targetX - 34, y: targetY - 12, text: data.targetCardinality }] as label}
      {#if label.text}<g transform={`translate(${label.x},${label.y})`}
          ><rect x="-24" y="-12" width="48" height="23" rx="5" /><text
            text-anchor="middle"
            dominant-baseline="central">{String(label.text)}</text
          ></g
        >{/if}
    {/each}
  </g>
{/if}

<style>
  .cardinality {
    pointer-events: auto;
  }
  rect {
    fill: #121c18;
    stroke: #668774;
    stroke-width: 1;
  }
  text {
    fill: #d8eddf;
    font:
      600 12px 'IBM Plex Mono',
      monospace;
  }
</style>
