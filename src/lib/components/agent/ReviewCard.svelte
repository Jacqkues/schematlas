<script lang="ts">
  import type { Review } from '$lib/services/agent';
  let { review, ondecide }: { review: Review; ondecide: (option: string | null) => Promise<void> } =
    $props();
  let busy = $state(false);
  async function decide(option: string | null) {
    busy = true;
    try {
      await ondecide(option);
    } finally {
      busy = false;
    }
  }
</script>

<section
  class="my-[18px] rounded-[9px] border border-[#59675d] bg-[#181c1a] p-3.5"
  aria-label="Permission request"
>
  <span class="eyebrow text-[#afc8b7]">REVIEW REQUIRED · {review.kind}</span>
  <h3 class="text-[13px] [overflow-wrap:anywhere]">{review.title}</h3>
  <pre
    class="max-h-[250px] overflow-auto rounded-[5px] bg-[#101411] p-[9px] font-mono text-[11px] leading-relaxed [overflow-wrap:anywhere] whitespace-pre-wrap text-[#c1cec5]">{JSON.stringify(
      review.details,
      null,
      2,
    )}</pre>
  <div class="flex flex-wrap gap-1.5">
    {#each review.options as option}<button
        disabled={busy}
        class={['btn px-2.5 py-[7px] text-[10px]', option.kind === 'allow_once' && 'btn-primary']}
        onclick={() => decide(option.optionId)}>{option.name}</button
      >{/each}
    <button class="btn px-2.5 py-[7px] text-[10px]" disabled={busy} onclick={() => decide(null)}
      >Cancel</button
    >
  </div>
</section>
