<script lang="ts">
  import type { Node, NodeProps } from '@xyflow/svelte';
  import { GripVertical } from '@lucide/svelte';
  let {
    data,
  }: NodeProps<
    Node<
      {
        name: string;
        count: number;
        color: string;
        onmove: (delta: { x: number; y: number }) => void;
      },
      'canvasGroup'
    >
  > = $props();
  function keymove(event: KeyboardEvent) {
    const step = event.shiftKey ? 50 : 10;
    const delta = {
      ArrowLeft: { x: -step, y: 0 },
      ArrowRight: { x: step, y: 0 },
      ArrowUp: { x: 0, y: -step },
      ArrowDown: { x: 0, y: step },
    }[event.key];
    if (!delta) return;
    event.preventDefault();
    event.stopPropagation();
    data.onmove(delta);
  }
</script>

<div
  class="canvas-group-overlay"
  style:--group-color={data.color}
  style:border-color={`${data.color}80`}
  style:background={`${data.color}08`}
>
  <button
    type="button"
    class="canvas-group-label group-drag-handle"
    aria-label={`Move group ${data.name}`}
    title="Drag to move all members. Arrow keys move 10px; Shift moves 50px."
    onkeydown={keymove}
  >
    <GripVertical size={14} /><span></span>{data.name}<small>{data.count}</small>
  </button>
</div>
