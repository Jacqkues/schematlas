<script lang="ts">
  import { onDestroy } from 'svelte';
  import { renderMarkdown } from '$lib/services/markdown';
  let { text, onrender }: { text: string; onrender?: () => void } = $props();
  let html = $state('');
  let pending = '';
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    pending = text;
    // Stream at most ten Markdown renders per second, even when tokens arrive faster.
    if (timer === undefined)
      timer = setTimeout(() => {
        html = renderMarkdown(pending);
        onrender?.();
        timer = undefined;
      }, 100);
  });
  onDestroy(() => clearTimeout(timer));
</script>

<div class="markdown">{@html html}</div>
