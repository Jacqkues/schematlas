<script lang="ts">
  let { compact = false }: { compact?: boolean } = $props();
</script>

<div class="flex items-center gap-2.5 text-[22px] font-bold tracking-[-1px]">
  <img
    src="/atlas.svg"
    alt=""
    width="32"
    height="32"
    class="rounded-lg opacity-90"
  />{#if !compact}<span>schem<span class="font-normal">atlas</span></span>{/if}
</div>
