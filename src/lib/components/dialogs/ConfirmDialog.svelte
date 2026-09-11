<script lang="ts">
  import Modal from './Modal.svelte';
  let {
    title,
    message,
    onclose,
    onconfirm,
  }: { title: string; message: string; onclose: () => void; onconfirm: () => Promise<void> } =
    $props();
  let busy = $state(false);
  let error = $state('');
  async function confirm() {
    busy = true;
    try {
      await onconfirm();
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal {title} {onclose} {busy}
  ><p class="text-[13px] leading-[1.9] text-[#cbcdd0]">{message}</p>
  {#if error}<p role="alert" class="form-error">{error}</p>{/if}
  <div class="modal-footer">
    <button class="btn" onclick={onclose} disabled={busy}>Cancel</button><button
      class="btn btn-danger"
      onclick={confirm}
      disabled={busy}>{busy ? 'Removing…' : 'Delete'}</button
    >
  </div></Modal
>
