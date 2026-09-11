<script lang="ts">
  import Modal from './Modal.svelte';
  import { api } from '$lib/services/workspace';
  import type { Project } from '$lib/types';
  import { FileJson, ArrowRight, Upload } from '@lucide/svelte';
  let {
    projectId,
    onclose,
    onsave,
  }: { projectId: string; onclose: () => void; onsave: (p: Project) => void } = $props();
  let file = $state<File | null>(null);
  let busy = $state(false);
  let error = $state('');
  function choose(event: Event) {
    const selected = (event.currentTarget as HTMLInputElement).files?.[0];
    error = '';
    file = null;
    if (!selected) return;
    if (selected.size > 20 * 1024 * 1024) {
      error = 'Choose a JSON file smaller than 20 MB.';
      return;
    }
    file = selected;
  }
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (!file) {
      error = 'Choose an OpenAPI JSON file first.';
      return;
    }
    busy = true;
    error = '';
    try {
      onsave(await api.import(projectId, await file.text()));
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal
  title="Your API, connected."
  subtitle="Import a definition to explore endpoints, models, and references."
  {onclose}
  {busy}
>
  <form onsubmit={submit}>
    <label class="upload-zone" for="openapi-file"
      ><div class="tile-icon sage"><FileJson size={28} /></div>
      <strong>{file ? file.name : 'Choose an OpenAPI file'}</strong><span
        >{file
          ? `${(file.size / 1024).toFixed(1)} KB · Ready to import`
          : 'OpenAPI 3.x or Swagger 2.0 · JSON · Up to 20 MB'}</span
      ><span class="button"
        ><Upload size={15} />{file ? 'Choose another file' : 'Browse files'}</span
      ><input
        id="openapi-file"
        name="file"
        class="visually-hidden"
        type="file"
        accept=".json,application/json"
        onchange={choose}
      /></label
    >
    <p class="form-hint">
      Local schema references become connections. External references are reported without fetching
      remote files.
    </p>
    {#if error}<p role="alert" class="form-error">{error}</p>{/if}
    <div class="modal-footer">
      <button type="button" class="button" onclick={onclose} disabled={busy}>Cancel</button><button
        class="button primary"
        type="submit"
        disabled={busy}
        >{busy ? 'Mapping definition…' : 'Import definition'}<ArrowRight size={16} /></button
      >
    </div>
  </form>
</Modal>
