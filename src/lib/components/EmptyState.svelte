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
</script>

<div class="empty-state">
  <div class="empty-kicker"><span></span> A CLEARER VIEW OF YOUR SYSTEM</div>
  <h1>
    {#if hasProject}{name}<span>starts here.</span>{:else}Complex systems.<span
        >Clear connections.</span
      >{/if}
  </h1>
  <p class="empty-description">
    Bring your databases and APIs into one local workspace.<br />See the structure. Follow the
    relationships. Find your bearings.
  </p>
  {#if !hasProject}<button class="button primary large" onclick={oncreate}
      ><Plus size={17} /> Create your first project <ArrowRight size={17} /></button
    >{/if}
  <div class="onboarding-cards">
    <button class="onboarding-card" onclick={hasProject ? onconnect : oncreate}
      ><div class="card-top">
        <div class="tile-icon terracotta"><Database size={22} /></div>
        <ArrowUpRight size={18} />
      </div>
      <h2>Map your database</h2>
      <p>Tables, columns, keys, and the relationships that connect them.</p>
      <span>PostgreSQL · MySQL · SQLite · more</span></button
    >
    <button class="onboarding-card" onclick={hasProject ? onimport : oncreate}
      ><div class="card-top">
        <div class="tile-icon sage"><Braces size={22} /></div>
        <ArrowUpRight size={18} />
      </div>
      <h2>Explore your API</h2>
      <p>Turn an OpenAPI definition into a connected map of endpoints and models.</p>
      <span>OpenAPI 3.x · Swagger 2.0</span></button
    >
  </div>
  <button class="demo-button" disabled={busy} onclick={ondemo}
    ><Workflow size={16} />{busy
      ? 'Creating example…'
      : 'Take a look around with an example project'}<ArrowRight size={15} /></button
  >
  <div class="empty-coordinate">SCHEMATLAS <span> / </span> UNDERSTAND WHAT CONNECTS.</div>
</div>
