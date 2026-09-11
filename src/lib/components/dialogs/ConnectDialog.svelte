<script lang="ts">
  import { untrack } from 'svelte';
  import Modal from './Modal.svelte';
  import { api, desktop } from '$lib/services/workspace';
  import { databaseNames, type DatabaseKind, type Project, type Source } from '$lib/types';
  import { open } from '@tauri-apps/plugin-dialog';
  import { Database, FolderOpen, Eye, EyeOff, ShieldCheck, ArrowRight } from '@lucide/svelte';
  let {
    projectId,
    source,
    onclose,
    onsave,
  }: { projectId: string; source?: Source; onclose: () => void; onsave: (p: Project) => void } =
    $props();
  let kind = $state<DatabaseKind>(untrack(() => source?.databaseKind ?? 'postgres'));
  let name = $state(untrack(() => source?.name ?? ''));
  let dsn = $state('');
  let reveal = $state(false);
  let busy = $state(false);
  let error = $state('');
  const placeholders: Record<DatabaseKind, string> = {
    postgres: 'postgresql://user:password@localhost:5432/database?sslmode=require',
    mysql: 'mysql://user:password@localhost:3306/database?ssl-mode=REQUIRED',
    mariadb: 'mysql://user:password@localhost:3306/database?ssl-mode=REQUIRED',
    sqlite: '/Users/you/data/database.sqlite',
    mssql: 'Server=tcp:localhost,1433;Database=app;User ID=sa;Password=…;Encrypt=true',
  };
  async function browse() {
    try {
      if (!desktop) throw new Error('File selection is available in the desktop app.');
      const path = await open({
        multiple: false,
        directory: false,
        title: 'Choose SQLite database',
      });
      if (typeof path === 'string') dsn = path;
    } catch (e) {
      error = String(e);
    }
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    error = '';
    try {
      const p = await api.connect(
        projectId,
        name,
        { kind, connectionString: dsn.trim() },
        source?.id,
      );
      dsn = '';
      onsave(p);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title={source ? 'Reconnect database' : 'Connect a database'}
  subtitle="Discover the structure behind your data."
  {onclose}
  {busy}
>
  <form onsubmit={submit}>
    <fieldset class="mb-[25px] grid grid-cols-3 gap-2">
      <legend class="form-label mb-3">Database engine</legend
      >{#each Object.entries(databaseNames) as [value, label]}<label
          class="flex cursor-pointer items-center gap-[7px] rounded-md border border-line-soft bg-surface px-[9px] py-3 text-[10px] text-text transition-colors has-checked:border-soft"
          ><input
            type="radio"
            class="m-0 size-[11px] accent-accent"
            name="engine"
            {value}
            checked={kind === value}
            onchange={() => {
              kind = value as DatabaseKind;
              dsn = '';
            }}
          /><Database size={16} class="w-[13px]" />{label}</label
        >{/each}
    </fieldset>
    <label class="form-label" for="connection-name"
      >Connection name <span class="text-accent">*</span></label
    ><input
      id="connection-name"
      class="mb-[21px] field"
      name="name"
      bind:value={name}
      placeholder="e.g. Production database"
      maxlength="80"
      required
    />
    <label class="form-label" for="connection-string"
      >{kind === 'sqlite' ? 'Database file' : 'Connection string'}
      <span class="text-accent">*</span></label
    >
    <div class="mb-2 field flex items-center gap-1.5 py-1.5 pr-[7px] pl-3">
      <input
        id="connection-string"
        class="w-full min-w-0 border-0 bg-transparent py-[5px] font-mono text-[11px]"
        name="connection"
        type={kind === 'sqlite' || reveal ? 'text' : 'password'}
        autocomplete="off"
        bind:value={dsn}
        placeholder={placeholders[kind]}
        required
        maxlength="8192"
        spellcheck="false"
      />{#if kind === 'sqlite'}<button
          type="button"
          class="icon-btn"
          aria-label="Choose database file"
          onclick={browse}><FolderOpen size={17} /></button
        >{:else}<button
          type="button"
          class="icon-btn"
          aria-label={reveal ? 'Hide connection string' : 'Show connection string'}
          onclick={() => (reveal = !reveal)}
          >{#if reveal}<EyeOff size={17} />{:else}<Eye size={17} />{/if}</button
        >{/if}
    </div>
    <p class="mb-[22px] font-mono form-hint text-[8px] [overflow-wrap:anywhere]">
      {placeholders[kind]}
    </p>
    <div class="flex gap-2.5 rounded-md bg-surface p-3.5 text-soft">
      <ShieldCheck size={18} class="shrink-0" />
      <p class="text-[10px] leading-[1.8]">
        Only schema metadata is inspected. Credentials stay in memory for this session. Saved maps
        remain available offline.
      </p>
    </div>
    {#if error}<p role="alert" class="form-error">{error}</p>{/if}
    <div class="modal-footer">
      <button type="button" class="btn" disabled={busy} onclick={onclose}>Cancel</button><button
        type="submit"
        class="btn btn-primary"
        disabled={busy}
        >{busy ? 'Inspecting schema…' : 'Connect & map'}<ArrowRight size={16} /></button
      >
    </div>
  </form>
</Modal>
