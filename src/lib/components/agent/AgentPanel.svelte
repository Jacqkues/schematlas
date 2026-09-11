<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { X, ArrowUp, Square, PlugZap, FolderOpen, Terminal } from '@lucide/svelte';
  import { agent, type AgentSnapshot } from '$lib/services/agent';
  import { desktop } from '$lib/services/workspace';
  import AgentDiscovery from './AgentDiscovery.svelte';
  import ReviewCard from './ReviewCard.svelte';
  import AgentActivity from './AgentActivity.svelte';
  import AgentMarkdown from './AgentMarkdown.svelte';
  import { motion } from '$lib/motion';
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
  const label = 'mt-5 mb-[7px] block text-[11px] text-soft';
  const input =
    'w-full rounded-[7px] border border-line-soft bg-field p-2.5 font-mono text-[11px] text-ink';
  const note = 'py-1 text-[11px] leading-relaxed text-muted';
  const messageText =
    'my-[7px] text-xs leading-[1.8] whitespace-pre-wrap text-ink [overflow-wrap:anywhere]';
  async function showLatest(behavior: ScrollBehavior = 'instant') {
    await tick();
    transcript?.scrollTo({ top: transcript.scrollHeight, behavior });
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
  function composerKeydown(event: KeyboardEvent) {
    // Enter sends; Shift+Enter (or any other modifier) keeps inserting a newline.
    // Ignore Enter that confirms an IME composition.
    if (
      event.key !== 'Enter' ||
      event.shiftKey ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.isComposing
    )
      return;
    event.preventDefault();
    if (snapshot?.status === 'ready') void send();
  }
  async function send(event?: SubmitEvent) {
    event?.preventDefault();
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

<aside
  class="agent-panel flex min-h-0 flex-col border-l border-line-soft bg-surface text-ink"
  aria-label="Local coding agent"
>
  <header class="flex items-center justify-between px-5 pt-6 pb-[15px]">
    <div>
      <span class="eyebrow">ACP SESSION</span>
      <h2 class="mt-[9px] flex items-center gap-[9px] text-lg font-bold">
        <Terminal size={18} />Local agent
      </h2>
    </div>
    <button class="icon-btn" aria-label="Close agent panel" onclick={onclose}
      ><X size={18} /></button
    >
  </header>
  <div class="flex items-center gap-[7px] border-y border-line px-5 py-3 text-[11px]">
    <span class="status-dot"></span>{projectName}<small
      class="ml-auto text-[10px] text-soft capitalize">{snapshot?.status ?? 'Not connected'}</small
    >
  </div>
  {#if !desktop}<p class="p-5 text-[11px] leading-relaxed text-muted">
      Agent sessions run in the desktop app.
    </p>
  {:else if !connected}
    <form class="overflow-auto p-[22px]" onsubmit={connect}>
      <AgentDiscovery
        selected={executable}
        onselect={(installed) => {
          executable = installed.executable;
          args = JSON.stringify(installed.args);
          rememberPreset();
        }}
      />
      <h3 class="text-base font-semibold">Your coding agent, in context.</h3>
      <p class="text-xs leading-relaxed text-soft">
        Connect an installed ACP agent or adapter. This session can inspect this project’s databases
        and APIs.
      </p>
      <label class={label} for="agent-executable">ACP executable</label>
      <div class="flex gap-[5px]">
        <input
          id="agent-executable"
          class={[input, 'min-w-0']}
          placeholder="/absolute/path/to/agent"
          bind:value={executable}
          required
        /><button
          type="button"
          class="icon-btn"
          aria-label="Choose agent executable"
          onclick={() => browse(false)}><FolderOpen size={16} /></button
        >
      </div>
      <label class={label} for="agent-args"
        >Arguments <small class="ml-[5px] text-muted">JSON array</small></label
      ><input id="agent-args" class={input} bind:value={args} placeholder={'["--acp"]'} required />
      <label class={label} for="agent-cwd">Working directory</label>
      <div class="flex gap-[5px]">
        <input
          id="agent-cwd"
          class={[input, 'min-w-0']}
          name="workingDirectory"
          aria-describedby="agent-directory-help"
          placeholder="/absolute/path/to/project"
          bind:value={cwd}
          required
        /><button
          type="button"
          class="icon-btn"
          aria-label="Choose working directory"
          onclick={() => browse(true)}><FolderOpen size={16} /></button
        >
      </div>
      <p class={note} id="agent-directory-help">
        {loadingDirectory
          ? 'Preparing your project folder…'
          : 'Your project folder is selected automatically. Choose an existing repository to use it instead; your choice is remembered, as is the last executable you connected.'}
      </p>
      <button class="mt-[22px] btn btn-primary" disabled={busy || (loadingDirectory && !cwd)}
        ><PlugZap size={15} />{busy ? 'Connecting…' : 'Connect agent'}</button
      >
      <p class={note}>
        Use an ACP-compatible adapter, not a regular interactive CLI. SQL and HTTP requests ask for
        your approval here. Agent file operations follow its own permission settings.
      </p>
    </form>
  {:else}
    <div class="flex items-center justify-between px-5 py-[13px] text-xs">
      <strong>{snapshot?.agentName}</strong><button
        class="btn px-2.5 py-1.5 text-[10px]"
        onclick={() => perform(() => agent.disconnect(projectId))}>Disconnect</button
      >
    </div>
    {#if snapshot?.status === 'authentication'}<div class="p-[22px]">
        <p class="text-xs leading-relaxed text-soft">
          Authenticate with your agent to start a session.
        </p>
        {#each snapshot.authMethods as method}<button
            class="btn"
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
      class="min-h-[100px] flex-1 overflow-auto overscroll-contain px-[18px] pt-2 pb-5"
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
      {#if !snapshot?.messages.length}<div class="px-2 py-[45px] text-soft">
          <Terminal size={26} />
          <h3 class="text-base font-semibold">Ask about your architecture.</h3>
          <p class="text-xs leading-[1.9] text-muted">
            “Group my tables by domain and arrange the map.”<br />“Find the API endpoint for
            creating an order.”
          </p>
        </div>{/if}
      {#each snapshot?.messages ?? [] as message (message.id)}<article
          class={[
            'my-[18px] [contain-intrinsic-size:auto_100px] [content-visibility:auto]',
            message.role === 'user' &&
              'rounded-[10px] border border-line-soft bg-surface px-3.5 py-3',
          ]}
        >
          <span class="text-[10px] font-semibold text-muted"
            >{message.role === 'user'
              ? 'You'
              : message.role === 'tool'
                ? 'Tool'
                : snapshot?.agentName}{message.status ? ` · ${message.status}` : ''}</span
          >
          {#if message.role === 'tool'}
            <details>
              <summary
                class="mt-1.5 cursor-pointer truncate font-mono text-[11px] leading-relaxed text-soft"
                >{message.text.split('\n')[0]}</summary
              >
              <p
                class={[messageText, 'max-h-[200px] overflow-auto font-mono text-[11px] text-soft']}
              >
                {message.text}
              </p>
            </details>
          {:else if message.role === 'assistant'}
            <AgentMarkdown
              text={message.text}
              onrender={() => {
                if (!unreadUpdates) void showLatest();
              }}
            />
          {:else}
            <p class={messageText}>{message.text}</p>
          {/if}
        </article>{/each}
    </div>
    {#if unreadUpdates}
      <button
        class="mx-4 mb-3 btn self-center text-[11px]"
        transition:fade={motion.fade()}
        onclick={() => showLatest('smooth')}>Latest activity ↓</button
      >
    {/if}
    {#if snapshot?.reviews.length}<div
        class="max-h-[40%] shrink-0 overflow-auto border-t border-line px-4 pb-3"
        aria-label="Pending approvals"
      >
        {#each snapshot.reviews as review (review.id)}<ReviewCard
            {review}
            ondecide={async (option) => {
              await perform(() => agent.decide(projectId, review.id, option));
            }}
          />{/each}
      </div>{/if}
    <form
      class="mx-4 mb-4 rounded-[11px] border border-line-strong bg-surface-3 p-3"
      onsubmit={send}
    >
      <label class="sr-only" for="agent-prompt">Message your agent</label><textarea
        id="agent-prompt"
        class="max-h-[200px] w-full resize-y border-0 bg-transparent text-xs leading-relaxed text-ink outline-none focus-visible:outline-none"
        bind:value={prompt}
        rows="3"
        maxlength="65536"
        placeholder="Ask about this project…"
        aria-describedby="agent-composer-help"
        onkeydown={composerKeydown}
        disabled={snapshot?.status !== 'ready'}></textarea>
      <div class="flex items-center justify-between">
        <span id="agent-composer-help" class="text-[10px] text-soft"
          >Enter sends · Shift+Enter for a new line</span
        >{#if ['running', 'cancelling'].includes(snapshot?.status ?? '')}<button
            type="button"
            class="btn"
            onclick={() => perform(() => agent.cancel(projectId))}><Square size={13} />Stop</button
          >{:else}<button
            class="btn btn-primary"
            aria-label="Send message"
            disabled={snapshot?.status !== 'ready' || !prompt.trim()}><ArrowUp size={16} /></button
          >{/if}
      </div>
    </form>
  {/if}
  {#if error || snapshot?.error}<p
      class="mx-[18px] form-error mb-[18px] max-h-[140px] overflow-auto text-xs"
      role="alert"
    >
      {error || snapshot?.error}
    </p>{/if}
</aside>
