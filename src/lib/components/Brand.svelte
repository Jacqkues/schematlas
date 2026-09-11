<script lang="ts">
  let { compact = false }: { compact?: boolean } = $props();
</script>

<div class="brand">
  <img src="/atlas.svg" alt="" width="32" height="32" />{#if !compact}<span
      >schem<span class="brand-light">atlas</span></span
    >{/if}
</div>
