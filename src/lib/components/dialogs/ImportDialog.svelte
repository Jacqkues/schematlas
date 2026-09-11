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
    <label
      class="relative mb-4 flex cursor-pointer flex-col items-center rounded-[9px] border border-dashed border-soft bg-surface px-[15px] pt-[35px] pb-[25px] transition-colors focus-within:outline-2 focus-within:outline-offset-3 focus-within:outline-accent hover:bg-[#0e1013]"
      for="openapi-file"
      ><div class="tile"><FileJson size={28} /></div>
      <strong class="mt-[18px] max-w-full text-sm font-medium [overflow-wrap:anywhere]"
        >{file ? file.name : 'Choose an OpenAPI file'}</strong
      ><span class="mt-2.5 mb-5 text-[9px] text-soft"
        >{file
          ? `${(file.size / 1024).toFixed(1)} KB · Ready to import`
          : 'OpenAPI 3.x or Swagger 2.0 · JSON · Up to 20 MB'}</span
      ><span class="btn text-[10px]"
        ><Upload size={15} />{file ? 'Choose another file' : 'Browse files'}</span
      ><input
        id="openapi-file"
        name="file"
        class="sr-only"
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
      <button type="button" class="btn" onclick={onclose} disabled={busy}>Cancel</button><button
        class="btn btn-primary"
        type="submit"
        disabled={busy}
        >{busy ? 'Mapping definition…' : 'Import definition'}<ArrowRight size={16} /></button
      >
    </div>
  </form>
</Modal>
