<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { slide } from 'svelte/transition';
  import { motion } from '$lib/motion';
  let { children }: { children: Snippet } = $props();
  let width = $state(420);
  let viewport = $state(1440);
  let dragging = $state(false);
  const minimum = 320;
  let maximum = $derived(Math.max(minimum, viewport - (viewport > 1100 ? 640 : 96)));
  let actual = $derived(Math.min(maximum, Math.max(minimum, width)));
  let origin: { x: number; width: number } | null = null;
  function save() {
    try {
      localStorage.setItem('atlas.chat.width', String(actual));
    } catch {
      /* Private storage can be unavailable. */
    }
  }
  onMount(() => {
    try {
      const saved = Number(localStorage.getItem('atlas.chat.width'));
      if (saved >= minimum) width = saved;
    } catch {
      /* Use the default width. */
    }
  });
  function start(event: PointerEvent) {
    if (event.button !== 0) return;
    origin = { x: event.clientX, width: actual };
    dragging = true;
    event.currentTarget instanceof HTMLElement &&
      event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }
  function move(event: PointerEvent) {
    if (origin)
      width = Math.max(minimum, Math.min(maximum, origin.width + origin.x - event.clientX));
  }
  function stop() {
    origin = null;
    dragging = false;
    save();
  }
  function key(event: KeyboardEvent) {
    const step = event.shiftKey ? 80 : 20;
    if (event.key === 'ArrowLeft') width = Math.min(maximum, actual + step);
    else if (event.key === 'ArrowRight') width = Math.max(minimum, actual - step);
    else if (event.key === 'Home') width = minimum;
    else if (event.key === 'End') width = maximum;
    else return;
    event.preventDefault();
    save();
  }
</script>

<svelte:window bind:innerWidth={viewport} />
<!-- The panel slides open along its width; the child keeps its final width so text never reflows mid-animation. -->
<div
  class={[
    'relative flex min-h-0 shrink-0 max-[1100px]:absolute max-[1100px]:inset-y-0 max-[1100px]:right-0 max-[1100px]:z-30 max-[1100px]:shadow-[-10px_0_30px_#0005] [&>.agent-panel]:w-(--chat-width) [&>.agent-panel]:shrink-0',
    dragging && 'select-none',
  ]}
  style:width={`${actual}px`}
  style:--chat-width={`${actual}px`}
  transition:slide={motion.slide()}
>
  <!-- WAI-ARIA window splitter: a focusable separator supports pointer and keyboard resizing. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class={[
      'absolute inset-y-0 -left-[5px] z-40 w-2.5 cursor-col-resize touch-none after:absolute after:left-1 after:h-full after:w-0.5 after:bg-transparent after:transition-colors hover:after:bg-accent-muted focus-visible:after:bg-accent-muted',
      dragging && 'after:bg-accent-muted',
    ]}
    role="separator"
    tabindex="0"
    aria-label="Resize agent chat"
    aria-orientation="vertical"
    aria-valuemin={minimum}
    aria-valuemax={maximum}
    aria-valuenow={Math.round(actual)}
    onpointerdown={start}
    onpointermove={move}
    onpointerup={stop}
    onpointercancel={stop}
    onlostpointercapture={stop}
    onkeydown={key}
    ondblclick={() => {
      width = 420;
      save();
    }}
  ></div>
  {@render children()}
</div>
