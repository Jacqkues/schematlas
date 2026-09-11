<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
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
<div class="resizable-chat" class:dragging style:width={`${actual}px`}>
  <!-- WAI-ARIA window splitter: a focusable separator supports pointer and keyboard resizing. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="chat-resizer"
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

<style>
  .resizable-chat {
    position: relative;
    flex-shrink: 0;
    min-height: 0;
    display: flex;
  }
  .chat-resizer {
    position: absolute;
    z-index: 40;
    left: -5px;
    top: 0;
    bottom: 0;
    width: 10px;
    cursor: col-resize;
    touch-action: none;
  }
  .chat-resizer::after {
    content: '';
    position: absolute;
    left: 4px;
    height: 100%;
    width: 2px;
  }
  .chat-resizer:hover::after,
  .chat-resizer:focus-visible::after,
  .dragging .chat-resizer::after {
    background: #91bf9b;
  }
  .dragging {
    user-select: none;
  }
  .resizable-chat :global(.agent-panel) {
    position: static;
    width: 100%;
    min-width: 0;
    max-width: none;
  }
  @media (max-width: 1100px) {
    .resizable-chat {
      position: absolute;
      right: 0;
      top: 0;
      bottom: 0;
      z-index: 30;
      box-shadow: -10px 0 30px #0005;
    }
  }
</style>
