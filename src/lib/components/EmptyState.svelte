<script lang="ts">
  import { ArrowUpRight, Plus, Database, Braces, ArrowRight, Workflow } from '@lucide/svelte';
  let {
    hasProject,
    name,
    oncreate,
    onconnect,
    onimport,
    ondemo,
    busy = false,
  }: {
    hasProject: boolean;
    name?: string;
    oncreate: () => void;
    onconnect: () => void;
    onimport: () => void;
    ondemo: () => void;
    busy?: boolean;
  } = $props();
  const short = '[@media(max-height:800px)]';
</script>

{#snippet card(title: string, text: string, tags: string, api: boolean, onclick: () => void)}
  <button
    class="rounded-[9px] border border-line-soft bg-surface p-[23px] px-[25px] text-left transition-[transform,border-color,background-color] duration-200 hover:-translate-y-[3px] hover:border-[#75777a] hover:bg-surface-2 max-[1000px]:p-[18px] [@media(max-height:800px)]:px-[22px] [@media(max-height:800px)]:py-[18px]"
    {onclick}
  >
    <div class="flex items-center justify-between text-[#b4b6b9]">
      <div class="tile">
        {#if api}<Braces size={22} />{:else}<Database size={22} />{/if}
      </div>
      <ArrowUpRight size={18} />
    </div>
    <h2
      class="mt-[23px] mb-2.5 text-[17px] font-[580] tracking-[-0.4px] [@media(max-height:800px)]:mt-4"
    >
      {title}
    </h2>
    <p class="max-w-[270px] text-xs leading-[1.75] text-[#aeb0b3]">{text}</p>
    <span
      class="mt-6 block border-t border-line-soft pt-3.5 font-mono text-[9px] tracking-[-0.2px] text-muted"
      >{tags}</span
    >
  </button>
{/snippet}

<div
  class="relative m-auto w-full max-w-[1120px] animate-rise-in px-[70px] pt-[60px] pb-[38px] max-[1180px]:p-10 [@media(max-height:800px)]:pt-[30px] [@media(max-height:800px)]:pb-5"
>
  <div
    class="mb-[25px] flex items-center gap-2.5 font-mono text-[10px] tracking-[1.5px] text-muted [@media(max-height:800px)]:mb-[18px]"
  >
    <span class="h-px w-[26px] bg-accent"></span> A CLEARER VIEW OF YOUR SYSTEM
  </div>
  <h1
    class="max-w-[850px] text-[clamp(34px,3.7vw,58px)] leading-[1.1] font-[480] tracking-[-2.9px] [overflow-wrap:anywhere] text-[#ebedf0] max-[1000px]:text-[40px] [@media(max-height:800px)]:text-[42px]"
  >
    {#if hasProject}{name}<span class="block text-muted">starts here.</span>{:else}Complex systems.<span
        class="block text-muted">Clear connections.</span
      >{/if}
  </h1>
  <p
    class="mt-[22px] mb-[25px] text-sm leading-[1.9] text-[#acaeb1] [@media(max-height:800px)]:mt-[17px] [@media(max-height:800px)]:mb-5 [@media(max-height:800px)]:text-xs"
  >
    Bring your databases and APIs into one local workspace.<br />See the structure. Follow the
    relationships. Find your bearings.
  </p>
  {#if !hasProject}<button class="btn btn-large btn-primary" onclick={oncreate}
      ><Plus size={17} /> Create your first project <ArrowRight
        size={17}
        class="ml-[18px]"
      /></button
    >{/if}
  <div
    class="mt-[42px] grid max-w-[750px] grid-cols-2 gap-[18px] max-[1000px]:gap-3 [@media(max-height:800px)]:mt-[25px]"
  >
    {@render card(
      'Map your database',
      'Tables, columns, keys, and the relationships that connect them.',
      'PostgreSQL · MySQL · SQLite · more',
      false,
      hasProject ? onconnect : oncreate,
    )}
    {@render card(
      'Explore your API',
      'Turn an OpenAPI definition into a connected map of endpoints and models.',
      'OpenAPI 3.x · Swagger 2.0',
      true,
      hasProject ? onimport : oncreate,
    )}
  </div>
  <button
    class="mt-[25px] flex items-center gap-[9px] py-1 text-[11px] text-[#acaeb1] transition-colors hover:text-accent"
    disabled={busy}
    onclick={ondemo}
    ><Workflow size={16} />{busy
      ? 'Creating example…'
      : 'Take a look around with an example project'}<ArrowRight size={15} /></button
  >
  <div
    class="mt-[52px] font-mono text-[8px] tracking-[1.3px] text-muted [@media(max-height:800px)]:mt-7"
  >
    SCHEMATLAS <span class="mx-3"> / </span> UNDERSTAND WHAT CONNECTS.
  </div>
</div>
