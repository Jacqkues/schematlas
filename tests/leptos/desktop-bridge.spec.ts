import { test, expect, type Page } from '@playwright/test';
import sample from '../../ui/src/sample.json' with { type: 'json' };

async function mockDesktop(
  page: Page,
  options: { connected?: boolean; emptyAgents?: boolean } = {},
) {
  await page.addInitScript(
    ({ project, options }) => {
      const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();
      const calls: { command: string; args: any }[] = [];
      const snapshot = {
        projectId: project.id,
        status: 'ready',
        agentName: 'Fixture ACP agent',
        executable: '/tmp/fixture-acp',
        authMethods: [] as { id: string; name: string }[],
        messages: [
          {
            id: 'answer',
            role: 'assistant',
            text: '# Commerce domain\n\n**Customers** own orders.\n\n```sql\nSELECT 1;\n```\n\n<script>alert(1)</script>',
          },
        ],
        reviews: [],
      };
      const diagram = {
        version: 1,
        kind: 'architecture',
        title: 'Northstar architecture',
        nodes: [
          { id: 'api', kind: 'service', label: 'Orders API', code_refs: ['src/api.rs:3'] },
          { id: 'db', kind: 'store', label: 'Commerce DB' },
        ],
        edges: [{ id: 'api->db', source: 'api', target: 'db', label: 'writes' }],
        layout: { api: { x: 0, y: 0 }, db: { x: 420, y: 0 } },
      };
      const repo = {
        root: '/tmp/northstar/repository',
        isGit: true,
        diagrams: [
          {
            slug: 'architecture',
            path: '.schematlas/diagrams/architecture.json',
            title: 'Northstar architecture',
            hash: 'h1',
            kind: 'architecture',
            nodes: 2,
            edges: 1,
          },
        ],
        docs: [],
      };
      const toc = {
        root: repo.root,
        hash: 'toc-1',
        toc: {
          version: 1,
          items: [
            {
              title: 'Diagrams',
              children: [
                {
                  title: 'Northstar architecture',
                  file: 'diagrams/architecture.json',
                  type: 'diagram',
                },
              ],
            },
            ...project.sources.map((s) => ({ title: s.name, source: s.name, type: s.kind })),
          ],
        },
        issues: {},
        hashes: {},
        error: null,
      };
      const sync = {
        projectId: project.id,
        enabled: true,
        remote: 'origin',
        remoteUrl: 'git@github.com:northstar/shop.git',
        me: { key: 'aaaaaaaaaaaa', name: 'Ana' },
        status: 'idle',
        lastSync: new Date().toISOString(),
        events: [
          {
            id: 'c1',
            author: { key: 'bbbbbbbbbbbb', name: 'Bo Teammate' },
            lamport: 1,
            ts: new Date().toISOString(),
            kind: 'comment',
            body: { diagram: 'architecture', node: 'db', text: 'Is this the read replica?' },
          },
        ],
        presence: [
          {
            author: { key: 'bbbbbbbbbbbb', name: 'Bo Teammate' },
            ts: new Date().toISOString(),
            diagram: 'architecture',
          },
        ],
        pending: 0,
      };
      Object.assign(window, {
        testDesktop: {
          calls,
          connected: options.connected ?? true,
          agents: options.emptyAgents
            ? []
            : [
                {
                  id: 'claude',
                  name: 'Claude Code',
                  executable: '/tmp/claude',
                  args: [],
                  acpReady: false,
                  setupUrl: 'https://github.com/agentclientprotocol/claude-agent-acp',
                },
                {
                  id: 'codex',
                  name: 'Codex',
                  executable: '/tmp/codex-acp',
                  args: [],
                  acpReady: true,
                },
                {
                  id: 'gemini',
                  name: 'Gemini CLI',
                  executable: '/tmp/gemini',
                  args: ['--experimental-acp'],
                  acpReady: true,
                },
              ],
          project,
          snapshot,
          repo,
          toc,
          sync,
          emit: (name: string, payload: unknown) =>
            listeners.get(name)?.forEach((fn) => fn({ payload })),
        },
        __TAURI__: {
          core: {
            invoke: async (command: string, args: any) => {
              calls.push({ command, args });
              switch (command) {
                case 'list_projects':
                  return [project];
                case 'save_positions':
                  Object.assign(project.sources[0].positions, args.positions);
                  return structuredClone(project);
                case 'agent_working_directory':
                  return '/tmp/northstar/repository';
                case 'list_repo':
                  return repo;
                case 'get_toc':
                  return structuredClone(toc);
                case 'get_diagram':
                  return { slug: 'architecture', hash: repo.diagrams[0].hash, diagram };
                case 'sync_status':
                case 'sync_now':
                  return sync;
                case 'sync_post':
                  sync.events.push({
                    id: `event-${sync.events.length}`,
                    author: sync.me,
                    lamport: sync.events.length + 1,
                    ts: new Date().toISOString(),
                    kind: args.draft.kind,
                    body: args.draft.body,
                  });
                  return structuredClone(sync);
                case 'agent_status':
                  return (window as any).testDesktop.connected ? structuredClone(snapshot) : null;
                case 'agent_connect_installed':
                case 'agent_connect': {
                  const desktop = (window as any).testDesktop;
                  if (desktop.failConnect)
                    throw new Error('Agent could not connect. Refresh and try again.');
                  if (desktop.deferConnect)
                    await new Promise<void>((resolve) => {
                      desktop.finishConnect = resolve;
                    });
                  const selected = desktop.agents.find((a: any) => a.id === args.agentId);
                  if (command === 'agent_connect_installed' && !selected)
                    throw new Error('Agent is no longer available');
                  desktop.connected = true;
                  snapshot.status = desktop.needsAuth ? 'authentication' : 'ready';
                  snapshot.agentName = selected?.name ?? 'Custom agent';
                  snapshot.executable = selected?.executable ?? args.config.executable;
                  snapshot.messages = [];
                  snapshot.authMethods = desktop.needsAuth
                    ? [{ id: 'signin', name: 'Sign in' }]
                    : [];
                  desktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                }
                case 'open_agent_setup':
                  return null;
                case 'agent_disconnect':
                  (window as any).testDesktop.connected = false;
                  snapshot.status = 'disconnected';
                  (window as any).testDesktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                case 'agent_authenticate':
                  snapshot.status = 'ready';
                  (window as any).testDesktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                case 'agent_prompt':
                  if ((window as any).testDesktop.failPrompt) throw new Error('Agent disconnected');
                  return null;
                case 'discover_agents':
                  return structuredClone((window as any).testDesktop.agents);
                case 'plugin:dialog|save':
                  return '/tmp/schema.json';
                case 'plugin:dialog|open':
                  return '/tmp/northstar/repository';
                case 'plugin:window|set_theme':
                case 'plugin:window|set_background_color':
                case 'export_source':
                case 'agent_decide':
                  return null;
                default:
                  throw new Error(`Unexpected command: ${command}`);
              }
            },
          },
          event: {
            listen: async (name: string, handler: (event: { payload: unknown }) => void) => {
              if (!listeners.has(name)) listeners.set(name, new Set());
              listeners.get(name)!.add(handler);
              return () => listeners.get(name)!.delete(handler);
            },
          },
        },
      });
    },
    { project: sample, options },
  );
}

test('repository events reload TOC navigation and expose invalid configuration', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  const explorer = page.getByRole('navigation', { name: 'Project files' });
  await expect(explorer.getByRole('button', { name: 'Diagrams', exact: true })).toBeVisible();
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.toc.hash = 'toc-external';
    desktop.toc.toc.items[0].title = 'Architecture';
    desktop.emit('repo:update', desktop.project.id);
  });
  await expect(explorer.getByRole('button', { name: 'Architecture', exact: true })).toBeVisible();
  await expect(explorer.getByRole('button', { name: 'Diagrams', exact: true })).toHaveCount(0);
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.toc.error = 'Invalid toc.yaml: expected items';
    desktop.toc.toc.items = [];
    desktop.emit('repo:update', desktop.project.id);
  });
  await expect(page.getByRole('alert')).toContainText('Invalid toc.yaml');
  await expect(page.getByRole('button', { name: 'New folder', exact: true })).toBeDisabled();
  await expect(explorer.getByRole('button', { name: 'Architecture', exact: true })).toHaveCount(0);
});

test('native IPC, agent Markdown and review events survive canvas edits', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page);
  await page.goto('/');
  await expect(page.locator('.entity-node')).toHaveCount(6);
  expect(await page.evaluate(() => document.documentElement.scrollHeight <= innerHeight)).toBe(
    true,
  );
  await expect(page.getByText('Browser preview', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  await expect(page.getByRole('heading', { name: 'Commerce domain' })).toBeVisible();
  await expect(page.locator('.markdown strong')).toHaveText('Customers');
  await expect(page.locator('.markdown pre code')).toHaveText('SELECT 1;');
  await expect(page.locator('.markdown script')).toHaveCount(0);
  // Equal-length replacements must update the existing message, without remounting it.
  await page
    .locator('.markdown')
    .evaluate((el) => el.setAttribute('data-stream-marker', 'retained'));
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.snapshot.messages[0].text = '**Live 1**';
    desktop.emit('agent:update', structuredClone(desktop.snapshot));
    desktop.snapshot.messages[0].text = '**Live 2**';
    desktop.emit('agent:update', structuredClone(desktop.snapshot));
  });
  await expect(page.locator('.markdown strong')).toHaveText('Live 2');
  await expect(page.locator('.markdown')).toHaveAttribute('data-stream-marker', 'retained');
  const prompt = page.locator('textarea');
  await prompt.fill('Keep my unsent request');
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    const project = structuredClone(desktop.project);
    project.sources[0].groups[0].name = 'Updated by agent';
    desktop.emit('workspace:update', project);
  });
  await expect(
    page.getByRole('button', { name: 'Move group Updated by agent', exact: true }),
  ).toBeVisible();
  await expect(prompt).toHaveValue('Keep my unsent request');
  const splitter = page.getByRole('separator', { name: 'Resize agent chat' });
  await splitter.focus();
  await splitter.press('ArrowLeft');
  await expect(splitter).toHaveAttribute('aria-valuenow', '440');
  const splitBox = (await splitter.boundingBox())!;
  await page.mouse.move(splitBox.x + splitBox.width / 2, splitBox.y + 80);
  await page.mouse.down();
  await page.mouse.move(splitBox.x + splitBox.width / 2 - 60, splitBox.y + 80, { steps: 12 });
  await page.mouse.up();
  await expect(splitter).toHaveAttribute('aria-valuenow', '500');
  expect(await page.evaluate(() => localStorage.getItem('atlas.chat.width'))).toBe('500');
  await expect(page.locator('.markdown')).toHaveCSS('font-size', '13px');
  await expect(page.locator('.agent-panel').locator('..')).toHaveCSS('animation-name', 'none');

  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.emit('agent:update', {
      ...desktop.snapshot,
      status: 'waiting',
      reviews: [
        {
          id: 'query-review',
          title: 'Review SQL',
          kind: 'sql',
          details: { sql: 'SELECT 1;' },
          options: [
            { optionId: 'allow_once', name: 'Run once', kind: 'allow_once' },
            { optionId: 'reject_once', name: 'Reject', kind: 'reject_once' },
          ],
        },
      ],
    });
  });
  await expect(page.getByText('Review SQL', { exact: true })).toBeVisible();
  // The statement to approve reads as SQL, not as a line inside a JSON blob.
  await expect(page.locator('[aria-label="Permission request"] pre code')).toHaveText('SELECT 1;');
  await expect(
    page.getByRole('heading', { name: '1 request waiting for your approval' }),
  ).toBeVisible();
  await expect(page.locator('[aria-label="Permission request"] details')).toHaveCount(1);
  const decisions = () =>
    page.evaluate(() =>
      (window as any).testDesktop.calls.filter((c: any) => c.command === 'agent_decide'),
    );
  expect(await decisions()).toHaveLength(0);
  await page.getByRole('button', { name: 'Reject', exact: true }).click();
  await expect.poll(decisions).toEqual([
    {
      command: 'agent_decide',
      args: { projectId: sample.id, reviewId: 'query-review', option: 'reject_once' },
    },
  ]);
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.emit('agent:update', {
      ...desktop.snapshot,
      status: 'running',
      activity: 'thinking',
      turnStartedAt: Date.now() - 3000,
      lastActivityAt: Date.now(),
      reviews: [],
      messages: [
        { id: 'call-1', role: 'tool', text: 'get_schema\nsource=Commerce', status: 'pending' },
        { id: 'call-2', role: 'tool', text: 'search_schema\nquery=order', status: 'failed' },
        { id: 'call-3', role: 'tool', text: 'describe_table\ntable=orders', status: 'completed' },
      ],
    });
  });
  // Backend vocabulary never reaches the user, and a failed call is not just more grey text.
  await expect(page.locator('.agent-panel').getByText('Working', { exact: true })).toBeVisible();
  await expect(page.locator('[data-tool-status="failed"]')).toHaveCount(1);
  await expect(page.locator('[data-tool-status="failed"]').getByText('failed')).toBeVisible();
  await expect(page.locator('[data-tool-status="completed"]').getByText('done')).toBeVisible();
  // The next question can be written while the current turn is still running.
  await expect(prompt).toBeEnabled();
  await prompt.fill('And the payments domain?');
  await expect(page.locator('#agent-composer-help')).toHaveText('Working…');

  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await page.getByRole('button', { name: 'Source details and actions' }).click();
  await page.getByRole('button', { name: 'Export', exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).testDesktop.calls.some((c: any) => c.command === 'export_source'),
      ),
    )
    .toBe(true);
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).testDesktop.calls.some(
          (c: any) => c.command === 'plugin:window|set_theme' && c.args.value === 'light',
        ),
      ),
    )
    .toBe(true);
  expect(errors).toEqual([]);
});

test('repository diagrams show team presence and comments, and agent proposals are reviewable', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Northstar architecture' }).first().click();
  const canvas = page.getByLabel('Interactive diagram', { exact: true });
  await expect(canvas.locator('.diagram-node')).toHaveCount(2);
  await expect(page.locator('.diagram-edge')).toHaveCount(1);
  await expect(page.getByText('writes')).toBeVisible();
  // Bo is looking at this diagram and left a comment on the database node.
  await expect(page.getByTitle('Bo Teammate')).toBeVisible();
  await expect(canvas.locator('[data-node-id="db"]').getByTitle('Team comments')).toHaveText('1');
  await canvas.locator('[data-node-id="db"]').click();
  await page.getByRole('button', { name: 'Details', exact: true }).click();
  await expect(page.getByText('Is this the read replica?')).toBeVisible();
  await page.getByLabel('Comment on this node').fill('Yes, the replica.');
  await page.getByRole('button', { name: 'Post comment' }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).testDesktop.calls.some(
          (c: any) => c.command === 'sync_post' && c.args.draft.kind === 'comment',
        ),
      ),
    )
    .toBe(true);
  // An agent proposal shows what changes before anything is written.
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.snapshot.reviews = [
      {
        id: 'r1',
        title: 'Diagram · Payments',
        kind: 'diagram',
        details: {
          path: '.schematlas/diagrams/payments.json',
          isNew: true,
          summary: '+2 ~0 −0 nodes · +1 ~0 −0 edges',
          change: { addedNodes: ['API', 'Stripe'], addedEdges: ['API → Stripe (charges)'] },
        },
        options: [
          { optionId: 'allow', name: 'Allow once', kind: 'allow_once' },
          { optionId: 'reject', name: 'Reject', kind: 'reject_once' },
        ],
      },
    ];
    desktop.emit('agent:update', structuredClone(desktop.snapshot));
  });
  const review = page.getByRole('region', { name: 'Permission request' });
  await expect(
    review.getByText('.schematlas/diagrams/payments.json', { exact: true }),
  ).toBeVisible();
  await expect(
    review.getByRole('listitem').filter({ hasText: 'API → Stripe (charges)' }),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test('overview requests wait in the native composer and failed sends restore the draft', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await page.getByRole('button', { name: 'Map the architecture', exact: true }).click();
  const prompt = page.getByLabel('Message your agent', { exact: true });
  await expect(prompt).toHaveValue(/Map this repository/);
  const draft = await prompt.inputValue();
  const sent = () =>
    page.evaluate(() =>
      (window as any).testDesktop.calls.filter((c: any) => c.command === 'agent_prompt'),
    );
  expect(await sent()).toHaveLength(0);
  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await page.getByRole('button', { name: 'Local agent ACP' }).click();
  await expect(prompt).toHaveValue(draft);
  await page.evaluate(() => {
    (window as any).testDesktop.failPrompt = true;
  });
  await page.getByRole('button', { name: 'Send message' }).click();
  await expect(page.getByRole('alert')).toHaveText('Agent disconnected');
  await expect(prompt).toHaveValue(draft);
  await page.evaluate(() => {
    (window as any).testDesktop.failPrompt = false;
  });
  await page.getByRole('button', { name: 'Send message' }).click();
  await expect(prompt).toHaveValue('');
  await expect.poll(sent).toHaveLength(2);
  expect((await sent())[1].args.text).toBe(draft);
});

test('installed agents connect directly from chat and preserve the unsent draft', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page, { connected: false });
  await page.goto('/');
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  const panel = page.getByRole('complementary', { name: 'Local coding agent' });
  await expect(panel.getByRole('button', { name: 'Use Codex', exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Use Gemini CLI', exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Use Claude Code', exact: true })).toBeDisabled();
  await panel.getByRole('button', { name: 'Set up', exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).testDesktop.calls.some(
          (c: any) => c.command === 'open_agent_setup' && c.args.agentId === 'claude',
        ),
      ),
    )
    .toBe(true);
  await expect(panel.getByLabel('Executable', { exact: true })).toHaveCount(0);
  await expect(panel.getByText('Working directory', { exact: true })).toHaveCount(0);
  const composer = panel.getByLabel('Message your agent');
  await composer.fill('Explain the checkout flow.');
  await page.screenshot({ path: test.info().outputPath('agent-picker.png') });
  await expect(panel.getByRole('button', { name: 'Send message' })).toBeDisabled();
  await panel.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Codex',
  );
  await expect(panel.getByRole('button', { name: 'Send message' })).toBeEnabled();
  await expect(composer).toHaveValue('Explain the checkout flow.');
  await page.screenshot({ path: test.info().outputPath('agent-connected.png') });
  const calls = await page.evaluate(() => (window as any).testDesktop.calls);
  expect(calls.filter((c: any) => c.command === 'agent_connect_installed')).toEqual([
    { command: 'agent_connect_installed', args: { projectId: sample.id, agentId: 'codex' } },
  ]);
  expect(calls.some((c: any) => c.command === 'agent_prompt')).toBe(false);
  await panel.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await panel.getByRole('button', { name: 'Use Gemini CLI', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Gemini CLI',
  );
  await expect(composer).toHaveValue('Explain the checkout flow.');
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.snapshot.status = 'running';
    desktop.emit('agent:update', structuredClone(desktop.snapshot));
  });
  await expect(panel.getByRole('button', { name: 'Choose agent', exact: true })).toBeDisabled();
  expect(errors).toEqual([]);
});

test('agent connection failures can retry and sign-in stays inside chat', async ({ page }) => {
  await mockDesktop(page, { connected: false });
  await page.goto('/');
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  await page.evaluate(() => ((window as any).testDesktop.failConnect = true));
  await page.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Agent could not connect');
  await expect(page.getByRole('button', { name: 'Use Codex', exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.failConnect = false;
    desktop.needsAuth = true;
  });
  await page.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Sign-in needed',
  );
  await page.getByLabel('Message your agent').fill('Hello');
  await expect(page.getByRole('button', { name: 'Send message' })).toBeDisabled();
  await page.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Send message' })).toBeEnabled();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('empty discovery can refresh and custom agents use the project directory', async ({
  page,
}) => {
  await mockDesktop(page, { connected: false, emptyAgents: true });
  await page.goto('/');
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  await expect(page.getByText(/No agents found/)).toBeVisible();
  await page.getByRole('button', { name: 'Custom agent…', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Custom agent' });
  await dialog.getByLabel('Executable', { exact: true }).fill('/tmp/custom-acp');
  await dialog.getByLabel('Arguments (optional)').fill('invalid');
  await dialog.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(dialog.getByRole('alert')).toContainText('JSON array');
  await dialog.getByLabel('Arguments (optional)').fill('["--acp"]');
  await dialog.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const calls = await page.evaluate(() => (window as any).testDesktop.calls);
  expect(calls.filter((c: any) => c.command === 'agent_connect')).toEqual([
    {
      command: 'agent_connect',
      args: {
        projectId: sample.id,
        config: {
          executable: '/tmp/custom-acp',
          args: ['--acp'],
          cwd: '/tmp/northstar/repository',
        },
      },
    },
  ]);
  await page.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await page.evaluate(() =>
    (window as any).testDesktop.agents.push({
      id: 'codex',
      name: 'Codex',
      executable: '/tmp/codex-acp',
      acpReady: true,
    }),
  );
  await page.getByRole('button', { name: 'Refresh agents', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Use Codex', exact: true })).toBeVisible();
});

test('closing chat during connection keeps the draft and reconnecting panel receives the result', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page, { connected: false });
  await page.goto('/');
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  await page.getByLabel('Message your agent').fill('Keep this draft');
  await page.evaluate(() => ((window as any).testDesktop.deferConnect = true));
  await page.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Choose agent', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await page.getByRole('button', { name: /Local agent.*ACP/ }).click();
  await page.evaluate(() => (window as any).testDesktop.finishConnect());
  await expect(page.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Codex',
  );
  await expect(page.getByLabel('Message your agent')).toHaveValue('Keep this draft');
  await expect(page.getByRole('button', { name: 'Send message' })).toBeEnabled();
  expect(errors).toEqual([]);
});
