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

<section class="agent-discovery" aria-label="Installed agents">
  <div class="discovery-heading">
    <span class="eyebrow">ON THIS COMPUTER</span><button
      type="button"
      class="icon-button"
      aria-label="Rescan installed agents"
      disabled={busy}
      onclick={scan}><RefreshCw size={13} /></button
    >
  </div>
  {#if busy}<p>Looking for installed agents…</p>{:else}
    {#each agents as installed}
      <button
        type="button"
        class="detected-agent"
        disabled={!installed.acpReady}
        aria-pressed={installed.executable === selected}
        onclick={() => onselect(installed)}
        title={installed.executable}
      >
        <Terminal size={16} /><span
          ><strong>{installed.name}</strong><small
            >{installed.executable === selected
              ? 'Selected'
              : installed.acpReady
                ? 'ACP preset available'
                : 'Installed · ACP adapter needed'}</small
          ></span
        >{#if installed.executable === selected}<Check size={14} />{/if}
      </button>
    {:else}<p>No known agent executables found. You can choose a custom executable below.</p>{/each}
    {#if agents.some((a) => !a.acpReady)}<p>
        Standalone Claude and Codex CLIs need an ACP adapter. Detection never starts an agent.
      </p>{/if}
  {/if}
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
</section>
