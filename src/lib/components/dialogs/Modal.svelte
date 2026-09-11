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
  aria-labelledby="modal-title"
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onclose();
  }}
>
  <div class="modal-heading">
    <div>
      <span class="eyebrow">SCHEMATLAS</span>
      <h2 id="modal-title">{title}</h2>
      {#if subtitle}<p>{subtitle}</p>{/if}
    </div>
    <button class="icon-button" aria-label="Close dialog" disabled={busy} onclick={onclose}
      ><X size={19} /></button
    >
  </div>
  {@render children()}
</dialog>
