<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { X, ArrowUp, Square, PlugZap, FolderOpen, Terminal } from '@lucide/svelte';
  import { agent, type AgentSnapshot } from '$lib/services/agent';
  import { desktop } from '$lib/services/workspace';
  import AgentDiscovery from './AgentDiscovery.svelte';
  import ReviewCard from './ReviewCard.svelte';
  import AgentActivity from './AgentActivity.svelte';
  import AgentMarkdown from './AgentMarkdown.svelte';
  let {
    projectId,
    projectName,
    onclose,
  }: { projectId: string; projectName: string; onclose: () => void } = $props();
  let snapshot = $state.raw<AgentSnapshot | null>(null);
  let executable = $state('');
  let args = $state('[]');
  let cwd = $state('');
  let loadingDirectory = $state(true);
  let prompt = $state('');
  let busy = $state(false);
  let error = $state('');
  let transcript = $state<HTMLDivElement>();
  let unreadUpdates = $state(false);
  async function showLatest() {
    await tick();
    if (transcript) transcript.scrollTop = transcript.scrollHeight;
    unreadUpdates = false;
  }
  const connected = $derived(snapshot && !['disconnected', 'error'].includes(snapshot.status));
  /** Last chosen ACP executable and arguments. Browser storage can be unavailable; never fail on it. */
  const PRESET_KEY = 'atlas:agent-preset';
  function rememberPreset() {
    try {
      localStorage.setItem(PRESET_KEY, JSON.stringify({ executable, args }));
    } catch {
      /* ignore */
    }
  }
  function restorePreset() {
    try {
      const saved: unknown = JSON.parse(localStorage.getItem(PRESET_KEY) ?? 'null');
      if (
        saved &&
        typeof saved === 'object' &&
        typeof (saved as { executable?: unknown }).executable === 'string' &&
        typeof (saved as { args?: unknown }).args === 'string'
      ) {
        executable = (saved as { executable: string }).executable;
        args = (saved as { args: string }).args;
      }
    } catch {
      /* ignore */
    }
  }
  async function perform(task: () => Promise<unknown>) {
    error = '';
    try {
      await task();
    } catch (e) {
      error = String(e);
    }
  }
  onMount(() => {
    if (!desktop) return;
    if (!executable) restorePreset();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<AgentSnapshot>('agent:update', async (event) => {
      if (event.payload.projectId !== projectId || disposed) return;
      const bottom =
        !transcript ||
        transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 100;
      snapshot = event.payload;
      await tick();
      if (bottom) await showLatest();
      else unreadUpdates = true;
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    void agent
      .workingDirectory(projectId)
      .then((directory) => {
        if (!disposed && !cwd) cwd = directory;
      })
      .catch((e) => {
        if (!disposed) error = String(e);
      })
      .finally(() => {
        if (!disposed) loadingDirectory = false;
      });
    void agent
      .status(projectId)
      .then((value) => {
        if (!disposed && !snapshot) {
          snapshot = value;
          void showLatest();
        }
      })
      .catch((e) => (error = String(e)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  });
  async function connect(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    await perform(async () => {
      const parsed: unknown = JSON.parse(args);
      if (!Array.isArray(parsed) || !parsed.every((a) => typeof a === 'string'))
        throw new Error('Arguments must be a JSON array of strings.');
      await agent.connect(projectId, { executable, args: parsed, cwd });
      rememberPreset();
    });
    busy = false;
  }
  async function send(event: SubmitEvent) {
    event.preventDefault();
    if (!prompt.trim()) return;
    const text = prompt;
    prompt = '';
    await perform(() => agent.prompt(projectId, text));
  }
  async function browse(directory: boolean) {
    await perform(async () => {
      const path = await open({
        directory,
        multiple: false,
        title: directory ? 'Agent working directory' : 'ACP executable',
        ...(directory && cwd ? { defaultPath: cwd } : {}),
      });
      if (typeof path === 'string') {
        if (directory) cwd = path;
        else executable = path;
      }
    });
  }
</script>

<aside class="agent-panel" aria-label="Local coding agent">
  <header class="agent-heading">
    <div>
      <span class="eyebrow">ACP SESSION</span>
      <h2><Terminal size={18} />Local agent</h2>
    </div>
    <button class="icon-button" aria-label="Close agent panel" onclick={onclose}
      ><X size={18} /></button
    >
  </header>
  <div class="agent-project">
    <span class="status-dot"></span>{projectName}<small>{snapshot?.status ?? 'Not connected'}</small
    >
  </div>
  {#if !desktop}<p class="agent-note">Agent sessions run in the desktop app.</p>
  {:else if !connected}
    <form class="agent-connection" onsubmit={connect}>
      <AgentDiscovery
        selected={executable}
        onselect={(installed) => {
          executable = installed.executable;
          args = JSON.stringify(installed.args);
          rememberPreset();
        }}
      />
      <h3>Your coding agent, in context.</h3>
      <p>
        Connect an installed ACP agent or adapter. This session can inspect this project’s databases
        and APIs.
      </p>
      <label for="agent-executable">ACP executable</label>
      <div class="agent-path">
        <input
          id="agent-executable"
          placeholder="/absolute/path/to/agent"
          bind:value={executable}
          required
        /><button
          type="button"
          class="icon-button"
          aria-label="Choose agent executable"
          onclick={() => browse(false)}><FolderOpen size={16} /></button
        >
      </div>
      <label for="agent-args">Arguments <small>JSON array</small></label><input
        id="agent-args"
        bind:value={args}
        placeholder={'["--acp"]'}
        required
      />
      <label for="agent-cwd">Working directory</label>
      <div class="agent-path">
        <input
          id="agent-cwd"
          name="workingDirectory"
          aria-describedby="agent-directory-help"
          placeholder="/absolute/path/to/project"
          bind:value={cwd}
          required
        /><button
          type="button"
          class="icon-button"
          aria-label="Choose working directory"
          onclick={() => browse(true)}><FolderOpen size={16} /></button
        >
      </div>
      <p class="agent-note" id="agent-directory-help">
        {loadingDirectory
          ? 'Preparing your project folder…'
          : 'Your project folder is selected automatically. Choose an existing repository to use it instead; your choice is remembered, as is the last executable you connected.'}
      </p>
      <button class="button primary" disabled={busy || (loadingDirectory && !cwd)}
        ><PlugZap size={15} />{busy ? 'Connecting…' : 'Connect agent'}</button
      >
      <p class="agent-note">
        Use an ACP-compatible adapter, not a regular interactive CLI. SQL and HTTP requests ask for
        your approval here. Agent file operations follow its own permission settings.
      </p>
    </form>
  {:else}
    <div class="agent-session">
      <strong>{snapshot?.agentName}</strong><button
        class="button"
        onclick={() => perform(() => agent.disconnect(projectId))}>Disconnect</button
      >
    </div>
    {#if snapshot?.status === 'authentication'}<div class="agent-connection">
        <p>Authenticate with your agent to start a session.</p>
        {#each snapshot.authMethods as method}<button
            class="button"
            disabled={busy}
            onclick={async () => {
              busy = true;
              await perform(() => agent.authenticate(projectId, method.id));
              busy = false;
            }}>{method.name}</button
          >{/each}
      </div>{/if}
    {#if snapshot}<AgentActivity {snapshot} />{/if}
    <div
      class="agent-transcript"
      bind:this={transcript}
      onscroll={() => {
        if (
          transcript &&
          transcript.scrollHeight - transcript.scrollTop - transcript.clientHeight < 100
        )
          unreadUpdates = false;
      }}
      role="log"
      aria-label="Agent conversation"
      aria-live="polite"
    >
      {#if !snapshot?.messages.length}<div class="agent-empty">
          <Terminal size={26} />
          <h3>Ask about your architecture.</h3>
          <p>
            “Group my tables by domain and arrange the map.”<br />“Find the API endpoint for
            creating an order.”
          </p>
        </div>{/if}
      {#each snapshot?.messages ?? [] as message (message.id)}<article
          class="agent-message"
          class:user={message.role === 'user'}
          class:tool={message.role === 'tool'}
        >
          <span
            >{message.role === 'user'
              ? 'You'
              : message.role === 'tool'
                ? 'Tool'
                : snapshot?.agentName}{message.status ? ` · ${message.status}` : ''}</span
          >
          {#if message.role === 'tool'}
            <details class="agent-tool-details">
              <summary>{message.text.split('\n')[0]}</summary>
              <p>{message.text}</p>
            </details>
          {:else if message.role === 'assistant'}
            <AgentMarkdown
              text={message.text}
              onrender={() => {
                if (!unreadUpdates) void showLatest();
              }}
            />
          {:else}
            <p>{message.text}</p>
          {/if}
        </article>{/each}
    </div>
    {#if unreadUpdates}
      <button class="button agent-latest" onclick={showLatest}>Latest activity ↓</button>
    {/if}
    {#if snapshot?.reviews.length}<div class="agent-pending-reviews" aria-label="Pending approvals">
        {#each snapshot.reviews as review (review.id)}<ReviewCard
            {review}
            ondecide={async (option) => {
              await perform(() => agent.decide(projectId, review.id, option));
            }}
          />{/each}
      </div>{/if}
    <form class="agent-composer" onsubmit={send}>
      <label class="sr-only" for="agent-prompt">Message your agent</label><textarea
        id="agent-prompt"
        bind:value={prompt}
        rows="3"
        maxlength="65536"
        placeholder="Ask about this project…"
        disabled={snapshot?.status !== 'ready'}></textarea>
      <div>
        <span>Schema + canvas tools</span
        >{#if ['running', 'cancelling'].includes(snapshot?.status ?? '')}<button
            type="button"
            class="button"
            onclick={() => perform(() => agent.cancel(projectId))}><Square size={13} />Stop</button
          >{:else}<button
            class="button primary"
            aria-label="Send message"
            disabled={snapshot?.status !== 'ready' || !prompt.trim()}><ArrowUp size={16} /></button
          >{/if}
      </div>
    </form>
  {/if}
  {#if error || snapshot?.error}<p class="form-error agent-error" role="alert">
      {error || snapshot?.error}
    </p>{/if}
</aside>
