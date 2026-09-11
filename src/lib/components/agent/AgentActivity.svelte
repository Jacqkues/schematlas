<script lang="ts">
  import { onMount } from 'svelte';
  import type { AgentSnapshot } from '$lib/services/agent';

  let { snapshot }: { snapshot: AgentSnapshot } = $props();
  let now = $state(Date.now());
  const running = $derived(['running', 'cancelling'].includes(snapshot.status));
  const silence = $derived(Math.max(0, Math.floor((now - snapshot.lastActivityAt) / 1000)));
  const elapsed = $derived(Math.max(0, Math.floor((now - (snapshot.turnStartedAt ?? now)) / 1000)));
  const activeTools = $derived(
    snapshot.messages
      .slice(snapshot.messages.findLastIndex((message) => message.role === 'user'))
      .filter(
        (message) =>
          message.role === 'tool' && !['completed', 'failed'].includes(message.status ?? ''),
      ),
  );
  const label = $derived(
    snapshot.reviews.length
      ? 'Waiting for your approval'
      : snapshot.status === 'cancelling'
        ? 'Stopping…'
        : silence >= 30
          ? 'Waiting for agent activity'
          : snapshot.activity === 'thinking'
            ? 'Thinking…'
            : snapshot.activity === 'planning'
              ? 'Planning…'
              : snapshot.activity === 'compacting'
                ? 'Compacting context…'
                : snapshot.activity === 'responding'
                  ? 'Writing a response…'
                  : activeTools.length
                    ? `Working on ${activeTools.length} tool ${activeTools.length === 1 ? 'call' : 'calls'}…`
                    : 'Waiting for agent…',
  );
  onMount(() => {
    const timer = window.setInterval(() => {
      now = Date.now();
    }, 1000);
    return () => window.clearInterval(timer);
  });
  function duration(seconds: number) {
    return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  }
</script>

{#if running || snapshot.reviews.length}
  <div class="mx-4 mb-3 animate-fade-in rounded-lg border border-line bg-surface px-3 py-[11px]">
    <div class="flex items-center gap-2">
      <span
        class={[
          'size-1.5 shrink-0 rounded-full',
          snapshot.reviews.length ? 'bg-[#e0bd83]' : 'bg-accent',
        ]}
      ></span><strong class="text-[11px] font-medium text-ink" role="status">{label}</strong><time
        class="ml-auto font-mono text-[10px] whitespace-nowrap text-muted tabular-nums"
        >{duration(elapsed)}</time
      >
    </div>
    <p class="mt-[7px] text-[10px] leading-relaxed text-muted">
      {snapshot.reviews.length
        ? 'Review the request below to continue.'
        : silence >= 30
          ? `No update for ${duration(silence)}. You can stop this run if needed.`
          : 'Live activity from your agent. Canvas edits appear as they are saved.'}
    </p>
  </div>
{/if}
