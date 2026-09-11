<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { RefreshCw, Check, Terminal } from '@lucide/svelte';
  export interface InstalledAgent {
    name: string;
    executable: string;
    args: string[];
    acpReady: boolean;
    note: string;
  }
  let {
    selected = '',
    onselect,
  }: { selected?: string; onselect: (agent: InstalledAgent) => void } = $props();
  let agents = $state<InstalledAgent[]>([]);
  let busy = $state(true);
  let error = $state('');
  const note = 'my-2 text-[10px] leading-relaxed';
  async function scan() {
    busy = true;
    error = '';
    try {
      agents = await invoke<InstalledAgent[]>('discover_agents');
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  onMount(() => {
    void scan();
  });
</script>

<section
  class="mb-6 rounded-[9px] border border-line-strong bg-surface p-3"
  aria-label="Installed agents"
>
  <div class="flex items-center justify-between">
    <span class="eyebrow">ON THIS COMPUTER</span><button
      type="button"
      class="icon-btn"
      aria-label="Rescan installed agents"
      disabled={busy}
      onclick={scan}><RefreshCw size={13} /></button
    >
  </div>
  {#if busy}<p class={note}>Looking for installed agents…</p>{:else}
    {#each agents as installed}
      {@const active = installed.executable === selected}
      <button
        type="button"
        class="flex w-full items-center gap-2.5 border-t border-line px-2 py-3 text-left text-text transition-colors not-disabled:hover:bg-accent-soft not-disabled:hover:text-accent-text disabled:opacity-65 aria-pressed:text-accent-text"
        disabled={!installed.acpReady}
        aria-pressed={active}
        onclick={() => onselect(installed)}
        title={installed.executable}
      >
        <Terminal size={16} /><span class="flex-1"
          ><strong class="block text-xs">{installed.name}</strong><small
            class="mt-[5px] block text-[10px] text-accent-muted"
            >{active
              ? 'Selected'
              : installed.acpReady
                ? 'ACP preset available'
                : 'Installed · ACP adapter needed'}</small
          ></span
        >{#if active}<Check size={14} />{/if}
      </button>
    {:else}<p class={note}>
        No known agent executables found. You can choose a custom executable below.
      </p>{/each}
    {#if agents.some((a) => !a.acpReady)}<p class={note}>
        Standalone Claude and Codex CLIs need an ACP adapter. Detection never starts an agent.
      </p>{/if}
  {/if}
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
</section>
