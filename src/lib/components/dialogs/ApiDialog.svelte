<script lang="ts">
  import { untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Modal from './Modal.svelte';
  import type { Source, Project } from '$lib/types';
  let {
    projectId,
    source,
    onclose,
    onsave,
  }: {
    projectId: string;
    source: Source;
    onclose: () => void;
    onsave: (project: Project) => void;
  } = $props();
  let baseUrl = $state(untrack(() => source.apiBaseUrl ?? ''));
  let headers = $state('{}');
  let busy = $state(false);
  let error = $state('');
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = '';
    try {
      const parsed = JSON.parse(headers);
      if (
        !parsed ||
        Array.isArray(parsed) ||
        typeof parsed !== 'object' ||
        !Object.values(parsed).every((v) => typeof v === 'string')
      )
        throw new Error('Headers must be a JSON object with string values.');
      onsave(
        await invoke<Project>('configure_api', {
          projectId,
          sourceId: source.id,
          config: { baseUrl, headers: parsed },
        }),
      );
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title="Connect this API"
  subtitle="Choose the server your agent can send requests to."
  {onclose}
  {busy}
>
  <form onsubmit={submit}>
    <label for="api-url">Base URL</label><input
      id="api-url"
      type="url"
      required
      bind:value={baseUrl}
      placeholder="http://localhost:3000/v1"
    />
    <p class="form-hint">Operation paths append to this URL. Redirects are not followed.</p>
    <label for="api-headers">Headers <small>JSON object · held in memory</small></label><textarea
      id="api-headers"
      rows="4"
      bind:value={headers}
      spellcheck="false"
      autocomplete="off"
      placeholder={'{"Authorization":"Bearer …"}'}></textarea>
    <p class="form-hint">
      Headers are cleared when the app closes. Each agent request needs your approval.
    </p>
    {#if error}<p class="form-error" role="alert">{error}</p>{/if}
    <div class="modal-footer">
      <button class="button primary" disabled={busy}
        >{busy ? 'Saving…' : 'Save API connection'}</button
      >
    </div>
  </form>
</Modal>
