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

<div class="agent-markdown">{@html html}</div>

<style>
  .agent-markdown {
    color: #dbdde0;
    font-size: 12px;
    line-height: 1.75;
    overflow-wrap: anywhere;
  }
  .agent-markdown :global(p) {
    margin: 8px 0;
    white-space: normal;
  }
  .agent-markdown :global(h1),
  .agent-markdown :global(h2),
  .agent-markdown :global(h3),
  .agent-markdown :global(h4),
  .agent-markdown :global(h5),
  .agent-markdown :global(h6) {
    margin: 18px 0 8px;
    color: #edf1ee;
    font-size: 14px;
    line-height: 1.4;
  }
  .agent-markdown :global(ul),
  .agent-markdown :global(ol) {
    margin: 8px 0;
    padding-left: 21px;
  }
  .agent-markdown :global(li) {
    margin: 5px 0;
  }
  .agent-markdown :global(code) {
    font: 11px/1.6 var(--mono);
    background: #1b2023;
    border-radius: 3px;
    padding: 2px 4px;
  }
  .agent-markdown :global(pre) {
    max-width: 100%;
    overflow: auto;
    white-space: pre;
    background: #101416;
    border: 1px solid #262d30;
    border-radius: 7px;
    padding: 12px;
  }
  .agent-markdown :global(pre code) {
    padding: 0;
    background: none;
    border-radius: 0;
  }
  .agent-markdown :global(blockquote) {
    margin: 10px 0;
    padding-left: 12px;
    border-left: 2px solid #637b6b;
    color: #aab8b0;
  }
  .agent-markdown :global(table) {
    display: block;
    max-width: 100%;
    overflow: auto;
    border-collapse: collapse;
    font-size: 11px;
  }
  .agent-markdown :global(th),
  .agent-markdown :global(td) {
    border: 1px solid #303638;
    padding: 6px 8px;
    text-align: left;
  }
  .agent-markdown :global(th) {
    background: #171d19;
  }
  .agent-markdown :global(a) {
    color: #a7d4b4;
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .agent-markdown :global(hr) {
    border: 0;
    border-top: 1px solid #303638;
    margin: 16px 0;
  }
</style>
