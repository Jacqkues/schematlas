<script lang="ts">
  import { onMount, type Snippet } from 'svelte';
  import { X } from '@lucide/svelte';
  let {
    title,
    subtitle = '',
    onclose,
    busy = false,
    children,
  }: {
    title: string;
    subtitle?: string;
    onclose: () => void;
    busy?: boolean;
    children: Snippet;
  } = $props();
  let element: HTMLDialogElement;
  onMount(() => {
    element.showModal();
  });
</script>

<dialog
  bind:this={element}
  class="m-auto max-h-[90dvh] w-[560px] max-w-[calc(100vw-48px)] overflow-auto rounded-[13px] border border-line bg-surface p-[30px] text-ink shadow-[0_30px_120px_#0009] backdrop:bg-overlay backdrop:backdrop-blur-[4px] open:animate-dialog-in open:backdrop:animate-fade-in"
  aria-labelledby="modal-title"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onclose();
  }}
>
  <div class="mb-[27px] flex items-start justify-between gap-2.5">
    <div>
      <span class="eyebrow">SCHEMATLAS</span>
      <h2 id="modal-title" class="mt-2 text-[25px] font-medium tracking-[-0.8px]">{title}</h2>
      {#if subtitle}<p class="mt-[9px] text-xs leading-[1.8] text-soft">{subtitle}</p>{/if}
    </div>
    <button
      class="-mt-2.5 -mr-2.5 icon-btn"
      aria-label="Close dialog"
      disabled={busy}
      onclick={onclose}><X size={19} /></button
    >
  </div>
  {@render children()}
</dialog>
