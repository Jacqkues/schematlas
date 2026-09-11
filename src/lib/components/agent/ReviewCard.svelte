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

<section class="agent-review" aria-label="Permission request">
  <span class="eyebrow">REVIEW REQUIRED · {review.kind}</span>
  <h3>{review.title}</h3>
  <pre>{JSON.stringify(review.details, null, 2)}</pre>
  <div class="review-actions">
    {#each review.options as option}<button
        disabled={busy}
        class="button"
        class:primary={option.kind === 'allow_once'}
        onclick={() => decide(option.optionId)}>{option.name}</button
      >{/each}
    <button class="button" disabled={busy} onclick={() => decide(null)}>Cancel</button>
  </div>
</section>
