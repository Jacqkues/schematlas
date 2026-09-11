<script lang="ts">
  import { Sun, Moon } from '@lucide/svelte';
  import { appearance, setTheme } from '$lib/state/appearance.svelte';
</script>

<div class="mx-1 mb-4 flex items-center justify-between gap-3" role="group" aria-label="Appearance">
  <span class="text-[11px] text-muted">Appearance</span>
  <div class="flex gap-0.5 rounded-lg border border-line bg-surface-3 p-0.5">
    {#each ['dark', 'light'] as theme}
      <button
        type="button"
        class="flex items-center gap-1.5 rounded-md px-2 py-1.5 text-[10px] text-muted transition-colors hover:text-ink aria-pressed:bg-surface aria-pressed:text-ink aria-pressed:shadow-sm"
        aria-label={theme === 'dark' ? 'Dark theme' : 'Light theme'}
        aria-pressed={appearance.theme === theme}
        onclick={() => setTheme(theme === 'dark' ? 'dark' : 'light')}
      >
        {#if theme === 'dark'}<Moon size={12} />{:else}<Sun size={12} />{/if}
        {theme === 'dark' ? 'Dark' : 'Light'}
      </button>
    {/each}
  </div>
</div>
