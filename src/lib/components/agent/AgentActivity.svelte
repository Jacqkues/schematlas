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
  <div class="activity" class:waiting={snapshot.reviews.length > 0}>
    <div>
      <span class="activity-dot"></span><strong role="status">{label}</strong><time
        >{duration(elapsed)}</time
      >
    </div>
    <p>
      {snapshot.reviews.length
        ? 'Review the request below to continue.'
        : silence >= 30
          ? `No update for ${duration(silence)}. You can stop this run if needed.`
          : 'Live activity from your agent. Canvas edits appear as they are saved.'}
    </p>
  </div>
{/if}

<style>
  .activity {
    margin: 0 16px 12px;
    padding: 11px 12px;
    border: 1px solid #24292b;
    border-radius: 8px;
    background: #101315;
  }
  .activity > div {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  strong {
    font-size: 11px;
    font-weight: 500;
    color: #d7ded9;
  }
  time {
    margin-left: auto;
    color: #a4aba7;
    font: 10px var(--mono);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  p {
    margin: 7px 0 0;
    font-size: 10px;
    line-height: 1.6;
    color: #a4aba7;
  }
  .activity-dot {
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
    background: #9ccd9c;
  }
  .waiting .activity-dot {
    background: #e0bd83;
  }
</style>
