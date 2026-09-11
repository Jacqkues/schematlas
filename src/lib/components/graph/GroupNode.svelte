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

<!-- Only the title bar takes pointer events, so cards inside the overlay stay clickable. -->
<div
  class="pointer-events-none size-full rounded-[14px] border border-dashed"
  style:--group-color={data.color}
  style:border-color={`${data.color}80`}
  style:background={`${data.color}08`}
>
  <button
    type="button"
    class="group-drag-handle pointer-events-auto flex w-full cursor-grab touch-none items-center gap-[9px] rounded-t-[13px] bg-[#11161b] px-4 py-[13px] text-left font-mono text-xs font-semibold whitespace-nowrap text-[#d5dce2] transition-colors hover:bg-[#192027] focus-visible:-outline-offset-3 focus-visible:outline-(--group-color) active:cursor-grabbing"
    aria-label={`Move group ${data.name}`}
    title="Drag to move all members. Arrow keys move 10px; Shift moves 50px."
    onkeydown={keymove}
  >
    <GripVertical size={14} class="text-[#8c969e]" /><span
      class="size-1.5 rounded-sm bg-(--group-color)"
    ></span>{data.name}<small class="ml-2 opacity-55">{data.count}</small>
  </button>
</div>
