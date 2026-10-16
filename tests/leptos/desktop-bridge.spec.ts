import { test, expect, type Page } from '@playwright/test';
import { openProject } from './support';
import sample from '../../ui/src/sample.json' with { type: 'json' };

async function mockDesktop(
  page: Page,
  options: { connected?: boolean; emptyAgents?: boolean } = {},
) {
  await page.addInitScript(
    ({ project, options }) => {
      const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();
      const calls: { command: string; args: any }[] = [];
      // Projects added through the projects page, after the fixture project.
      const added: any[] = [];
      const snapshot = {
        projectId: project.id,
        status: 'ready',
        agentName: 'Fixture ACP agent',
        executable: '/tmp/fixture-acp',
        sessionId: 'fixture-1',
        models: [
          { id: 'fixture-fast', name: 'Fixture Fast' },
          { id: 'fixture-deep', name: 'Fixture Deep' },
        ],
        modes: [
          { id: 'ask', name: 'Ask first', description: 'Asks before changing anything.' },
          { id: 'auto', name: 'Auto', description: 'Acts on its own; asks when unsure.' },
        ],
        modeId: 'ask',
        modelId: 'fixture-fast',
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
      // The parser's model of `.schematlas/diagrams/architecture.mmd`, as the backend sends it.
      const diagram = {
        kind: 'flowchart',
        direction: 'LR',
        title: 'Northstar architecture',
        nodes: [
          {
            id: 'api',
            label: 'Orders API',
            annotation: { kind: 'service', code: [{ path: 'src/api.rs', line: 3 }] },
          },
          { id: 'db', label: 'Commerce DB', shape: 'cylinder' },
        ],
        edges: [
          { from: 'api', to: 'db', label: 'writes', stroke: 'normal', head: 'arrow', length: 1 },
        ],
        layout: {
          api: { x: 0, y: 0, width: 120, height: 60 },
          db: { x: 420, y: 0, width: 120, height: 60 },
        },
      };
      const text = 'flowchart LR\n  api(Orders API) -->|writes| db[(Commerce DB)]\n';
      const repo = {
        root: '/tmp/northstar/repository',
        isGit: true,
        diagrams: [
          {
            id: '.schematlas/diagrams/architecture.mmd',
            path: '.schematlas/diagrams/architecture.mmd',
            format: 'mermaid',
            title: 'Northstar architecture',
            hash: 'h1',
            kind: 'flowchart',
            nodes: 2,
            edges: 1,
            editable: 'full',
          },
          {
            id: '.schematlas/schema/shop.dbml',
            path: '.schematlas/schema/shop.dbml',
            format: 'dbml',
            title: 'Shop schema',
            hash: 'd1',
            kind: 'dbml',
            nodes: 2,
            edges: 1,
            editable: 'annotations',
          },
        ],
        docs: [],
      };
      // A DBML file, read by the backend as a diagram and as tables for the schema canvas.
      const field = (name: string, dataType: string, primaryKey = false) => ({
        name,
        dataType,
        nullable: !primaryKey,
        primaryKey,
        required: primaryKey,
        defaultValue: null,
        description: null,
      });
      const dbml = {
        id: '.schematlas/schema/shop.dbml',
        path: '.schematlas/schema/shop.dbml',
        format: 'dbml',
        title: 'Shop schema',
        hash: 'd1',
        editable: 'annotations',
        source: 'Table users {}',
        text: 'Table users {}',
        diagram: {
          kind: 'dbml',
          nodes: [
            { id: 'public.users', label: 'users', shape: 'table' },
            { id: 'public.orders', label: 'orders', shape: 'table' },
          ],
          edges: [{ from: 'public.orders', to: 'public.users', stroke: 'normal', head: 'none' }],
        },
        schema: {
          entities: [
            {
              id: 'public.users',
              name: 'users',
              namespace: 'public',
              kind: 'table',
              description: null,
              fields: [field('id', 'bigint', true), field('email', 'text')],
            },
            {
              id: 'public.orders',
              name: 'orders',
              namespace: 'public',
              kind: 'table',
              description: null,
              fields: [field('id', 'bigint', true), field('user_id', 'bigint')],
            },
          ],
          relations: [
            {
              id: 'ref-0-0',
              source: 'public.orders',
              target: 'public.users',
              sourceField: 'user_id',
              targetField: 'id',
              label: 'orders → users',
            },
          ],
          warnings: [],
        },
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
                  file: '.schematlas/diagrams/architecture.mmd',
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
            body: {
              diagram: '.schematlas/diagrams/architecture.mmd',
              node: 'db',
              text: 'Is this the read replica?',
            },
          },
        ],
        presence: [
          {
            author: { key: 'bbbbbbbbbbbb', name: 'Bo Teammate' },
            ts: new Date().toISOString(),
            diagram: '.schematlas/diagrams/architecture.mmd',
          },
        ],
        pending: 0,
      };
      Object.assign(window, {
        testDesktop: {
          calls,
          connected: options.connected ?? true,
          nextSession: 2,
          pendingSession: false,
          conversations:
            options.connected === false
              ? []
              : [
                  {
                    id: 'fixture-1',
                    label: 'Map checkout flow',
                    agentName: 'Fixture ACP agent',
                    available: true,
                  },
                ],
          agents: options.emptyAgents
            ? []
            : [
                {
                  id: 'claude',
                  name: 'Claude Code',
                  executable: '/tmp/claude',
                  args: [],
                  available: true,
                },
                {
                  id: 'codex',
                  name: 'Codex',
                  executable: '/tmp/codex',
                  args: [],
                  available: true,
                },
                {
                  id: 'gemini',
                  name: 'Gemini CLI',
                  executable: '/tmp/gemini',
                  args: ['--experimental-acp'],
                  available: true,
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
                  return [project, ...added];
                case 'create_project': {
                  const created: any = {
                    ...structuredClone(project),
                    id: `added-${added.length}`,
                    name: args.name,
                    description: args.description,
                    sources: [],
                  };
                  delete created.workingDirectory;
                  added.push(created);
                  return structuredClone(created);
                }
                case 'open_repository': {
                  const target = added.find((p) => p.id === args.projectId) ?? project;
                  target.workingDirectory = args.path;
                  return structuredClone(target);
                }
                case 'save_positions':
                  Object.assign(project.sources[0].positions, args.positions);
                  return structuredClone(project);
                case 'agent_working_directory':
                  return '/tmp/northstar/repository';
                case 'list_repo':
                  return repo;
                case 'get_toc':
                  return structuredClone(toc);
                case 'list_branches':
                  return {
                    current: 'feature',
                    reference: 'main',
                    all: ['feature', 'main', 'origin/main'],
                  };
                case 'repo_freshness': {
                  const f = (window as any).testDesktop.freshness;
                  const changed = f ? Object.keys(f.nodes).length : 0;
                  return {
                    branch: 'origin/main',
                    commit: 'f'.repeat(40),
                    diagrams: changed
                      ? {
                          '.schematlas/diagrams/architecture.mmd': { changed, broken: 0, local: 0 },
                        }
                      : {},
                  };
                }
                case 'diagram_freshness':
                  return (
                    (window as any).testDesktop.freshness ?? {
                      branch: 'origin/main',
                      commit: 'f'.repeat(40),
                      nodes: {},
                    }
                  );
                case 'review_node': {
                  (window as any).testDesktop.freshness = {
                    branch: 'origin/main',
                    commit: 'f'.repeat(40),
                    nodes: {},
                  };
                  return {
                    id: args.id,
                    path: args.id,
                    format: 'mermaid',
                    title: 'Northstar architecture',
                    hash: 'h2',
                    editable: 'full',
                    source: text,
                    text,
                    diagram,
                  };
                }
                case 'edit_diagram':
                  if (args.id === dbml.id) return structuredClone(dbml);
                  throw new Error('Unexpected edit');
                case 'get_diagram':
                  if (args.id === dbml.id) return structuredClone(dbml);
                  return {
                    id: '.schematlas/diagrams/architecture.mmd',
                    path: '.schematlas/diagrams/architecture.mmd',
                    format: 'mermaid',
                    title: 'Northstar architecture',
                    hash: repo.diagrams[0].hash,
                    editable: 'full',
                    source: text,
                    text,
                    diagram,
                  };
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
                case 'agent_conversations':
                  return (window as any).testDesktop.conversations.map((r: any) => ({
                    ...r,
                    active: (window as any).testDesktop.connected && r.id === snapshot.sessionId,
                  }));
                case 'agent_reopen_conversation': {
                  const desktop = (window as any).testDesktop;
                  if (desktop.failReopen)
                    throw new Error('Stop the current turn before switching conversations.');
                  const row = desktop.conversations.find((r: any) => r.id === args.conversationId);
                  if (!row) throw new Error('Conversation not found');
                  desktop.connected = true;
                  snapshot.sessionId = row.id;
                  desktop.pendingSession = false;
                  snapshot.agentName = row.agentName;
                  snapshot.status = 'ready';
                  snapshot.messages = [
                    { id: 'reopened', role: 'assistant', text: `Reopened ${row.label}.` },
                  ];
                  desktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                }
                case 'agent_new_conversation': {
                  const desktop = (window as any).testDesktop;
                  if (desktop.failNewConversation)
                    throw new Error('Could not start a new conversation');
                  snapshot.sessionId = `fixture-${desktop.nextSession++}`;
                  desktop.pendingSession = true;
                  snapshot.messages = [];
                  desktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                }
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
                  snapshot.sessionId = `fixture-${desktop.nextSession++}`;
                  desktop.pendingSession = true;
                  snapshot.messages = [];
                  snapshot.models = [
                    { id: `${selected?.id ?? 'custom'}-fast`, name: 'Fast model' },
                    { id: `${selected?.id ?? 'custom'}-deep`, name: 'Deep model' },
                  ];
                  snapshot.modelId = snapshot.models[0].id;
                  snapshot.authMethods = desktop.needsAuth
                    ? [{ id: 'signin', name: 'Sign in' }]
                    : [];
                  desktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                }
                case 'agent_set_model':
                  if (!snapshot.models.some((m) => m.id === args.modelId))
                    throw new Error('Unknown model');
                  snapshot.modelId = args.modelId;
                  (window as any).testDesktop.emit('agent:update', structuredClone(snapshot));
                  return null;
                case 'agent_set_mode':
                  if (!snapshot.modes.some((m) => m.id === args.modeId))
                    throw new Error('Unknown mode');
                  snapshot.modeId = args.modeId;
                  (window as any).testDesktop.emit('agent:update', structuredClone(snapshot));
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
                  const desktop = (window as any).testDesktop;
                  if (desktop.pendingSession) {
                    desktop.conversations.push({
                      id: snapshot.sessionId,
                      label: 'Review database relationships',
                      agentName: snapshot.agentName,
                      available: true,
                    });
                    desktop.pendingSession = false;
                  }
                  snapshot.messages.push({ id: 'sent', role: 'user', text: args.text });
                  snapshot.messages.push({
                    id: 'reply',
                    role: 'assistant',
                    text: 'An agent answer.',
                  });
                  (window as any).testDesktop.emit('agent:update', structuredClone(snapshot));
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

test('nodes whose cited code changed on the reference branch say so, and can be reviewed', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  await page.evaluate(() => {
    (window as any).testDesktop.freshness = {
      branch: 'origin/main',
      commit: 'f'.repeat(40),
      nodes: {
        db: {
          status: 'changed',
          baseline: '1234567890abcdef',
          refs: [
            {
              reference: 'src/db.rs:3-4',
              status: 'changed',
              now: 'src/db.rs:4-5',
              patch: '@@ -3 +3 @@\n-pool = Pool(5)\n+pool = Pool(20)',
              commits: ['abc1234 Raise the pool size'],
              local: false,
            },
          ],
        },
      },
    };
  });
  await openProject(page);
  // The sidebar says which diagram has changed code, and its folder too.
  const files = page.getByRole('navigation', { name: 'Project files' });
  const flag = '1 node whose code changed on origin/main';
  await expect(files.getByLabel(flag).first()).toBeVisible();
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await page
    .getByRole('region', { name: 'Code changes', exact: true })
    .getByRole('button', { name: /Northstar architecture/ })
    .click();
  const canvas = page.getByLabel('Interactive diagram', { exact: true });
  const db = canvas.locator('[data-node-id="db"]');
  await expect(db.getByLabel('Cited code changed since this node was checked')).toBeVisible();
  await page.getByRole('button', { name: /1 node changed · origin\/main/ }).click();
  const changes = page.getByRole('region', { name: 'Code changes' });
  await expect(changes).toContainText('Code changed on origin/main');
  await expect(changes).toContainText('abc1234 Raise the pool size');
  await expect(changes).toContainText('→ src/db.rs:4-5');
  await changes.getByText('What changed').click();
  await expect(changes.getByText('+pool = Pool(20)')).toBeVisible();
  await changes.getByRole('button', { name: 'Ask agent to update' }).click();
  await expect(page.getByLabel('Message your agent')).toHaveValue(/Raise|pool = Pool\(20\)/);
  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await changes.getByRole('button', { name: 'Reviewed' }).click();
  await expect(db.getByLabel('Cited code changed since this node was checked')).toHaveCount(0);
  await expect(files.getByLabel(flag)).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).testDesktop.calls
        .filter((c: any) => c.command === 'review_node')
        .map((c: any) => c.args.node),
    ),
  ).toEqual(['db']);
});

test('the agent hears what is open and runs in the mode chosen for its harness', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
  await page
    .getByRole('navigation', { name: 'Project files' })
    .getByRole('button', { name: /Northstar architecture/ })
    .first()
    .click();
  const canvas = page.getByLabel('Interactive diagram', { exact: true });
  await canvas.locator('[data-node-id="db"]').click();
  await page.getByRole('button', { name: 'Ask your agent' }).first().click();
  const chat = page.getByRole('complementary', { name: 'Local coding agent' });
  await expect(chat.getByText('architecture.mmd', { exact: true })).toBeVisible();
  await expect(chat.getByText('· Commerce DB')).toBeVisible();
  const composer = chat.getByLabel('Message your agent');
  await composer.fill('What reads from this?');
  await composer.press('Enter');
  const sent = () =>
    page.evaluate(() =>
      (window as any).testDesktop.calls
        .filter((c: any) => c.command === 'agent_prompt')
        .map((c: any) => c.args.text),
    );
  await expect
    .poll(sent)
    .toEqual([
      'What reads from this?\n\nOpen in Schematlas: diagram `.schematlas/diagrams/architecture.mmd`, selected node `db` (\u201cCommerce DB\u201d).',
    ]);
  // Dismissed, the next message goes without it.
  await chat.getByRole('button', { name: "Don't send what is open" }).click();
  await composer.fill('Thanks');
  await composer.press('Enter');
  await expect.poll(async () => (await sent()).at(-1)).toBe('Thanks');
  // The harness's mode is one choice away, and remembered for this agent.
  await chat.getByRole('button', { name: 'Choose mode', exact: true }).click();
  await chat
    .getByRole('region', { name: 'Agent modes' })
    .getByRole('button', { name: /^Auto/ })
    .click();
  await expect(chat.getByRole('button', { name: 'Choose mode', exact: true })).toContainText(
    'Auto',
  );
  expect(
    await page.evaluate(() =>
      (window as any).testDesktop.calls
        .filter((c: any) => c.command === 'agent_set_mode')
        .map((c: any) => c.args.modeId),
    ),
  ).toEqual(['auto']);
  expect(
    await page.evaluate(() => localStorage.getItem('atlas:agent-mode:Fixture ACP agent')),
  ).toBe('auto');
});

test('the projects page adds a repository as a project named after its folder', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  const list = page.getByRole('list', { name: 'Projects' });
  await expect(list.getByRole('button')).toHaveCount(2);
  await page.getByRole('button', { name: 'Add repository', exact: true }).click();
  // The folder picker answered /tmp/northstar/repository: a project for it opens at once.
  await expect(page.getByRole('heading', { name: 'repository', exact: true })).toBeVisible();
  // The overview is just the prompt and past conversations; files live in the explorer.
  for (const section of ['Ask your agent', 'Conversations'])
    await expect(page.getByRole('region', { name: section, exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Diagrams', exact: true })).toHaveCount(0);
  const opened = await page.evaluate(() =>
    (window as any).testDesktop.calls
      .filter((c: any) => ['create_project', 'open_repository'].includes(c.command))
      .map((c: any) => [c.command, c.args.name ?? c.args.path]),
  );
  expect(opened).toEqual([
    ['create_project', 'repository'],
    ['open_repository', '/tmp/northstar/repository'],
  ]);
  // Adding the same folder again reopens that project instead of duplicating it.
  await page.getByRole('button', { name: 'All projects' }).first().click();
  await expect(list.getByRole('button', { name: /^repository/ })).toHaveCount(1);
  await page.getByRole('button', { name: 'Add repository', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'repository', exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).testDesktop.calls.filter((c: any) => c.command === 'create_project').length,
    ),
  ).toBe(1);
});

test('DBML files open in the schema canvas, with columns and column-level references', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
  // Files the TOC does not list are found by folder in the explorer.
  const unlisted = page.getByRole('region', { name: 'Repository diagrams' });
  await expect(unlisted.getByText('schema', { exact: true })).toBeVisible();
  await expect(unlisted.getByRole('button', { name: 'Shop schema', exact: true })).toBeVisible();
  await unlisted.getByRole('button', { name: 'Shop schema', exact: true }).click();
  const tables = page.locator('.entity-node');
  await expect(tables).toHaveCount(2);
  await expect(tables.filter({ hasText: 'users' })).toContainText('email');
  // The schema canvas arranges the tables and keeps the arrangement beside the file.
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).testDesktop.calls
          .filter((c: any) => c.command === 'edit_diagram')
          .flatMap((c: any) => c.args.edits.map((e: any) => Object.keys(e)[0])),
      ),
    )
    .toContain('Move');
  expect(errors).toEqual([]);
});

test('repository events reload TOC navigation and expose invalid configuration', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
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
  await openProject(page);
  await expect(page.locator('.entity-node')).toHaveCount(6);
  expect(await page.evaluate(() => document.documentElement.scrollHeight <= innerHeight)).toBe(
    true,
  );
  await expect(page.getByText('Browser preview', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: /Ask your agent/ }).click();
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
  await expect(page.locator('#agent-composer-help')).toBeVisible();
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
  await openProject(page);
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
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.snapshot.reviews = [
      {
        id: 'r1',
        title: 'Diagram · Payments',
        kind: 'diagram',
        details: {
          path: '.schematlas/diagrams/payments.mmd',
          isNew: true,
          summary: '+2 ~0 −0 nodes · +1 ~0 −0 edges',
          diff: '+flowchart LR\n+  api[API] -->|charges| psp[(Stripe)]\n',
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
    review.getByText('.schematlas/diagrams/payments.mmd', { exact: true }),
  ).toBeVisible();
  await expect(
    review.locator('.text-accent', { hasText: 'api[API] -->|charges| psp[(Stripe)]' }),
  ).toBeVisible();
  await expect(
    review.getByRole('listitem').filter({ hasText: 'API → Stripe (charges)' }),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test('overview requests wait in the home composer and failed sends restore the draft', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  await expect(page.locator('.agent-panel')).toBeVisible();
  await page.getByRole('button', { name: 'Map the architecture', exact: true }).click();
  // The suggestion goes to the home composer, not to the open conversation.
  await expect(page.locator('.agent-panel')).toHaveCount(0);
  const prompt = page.getByLabel('Message your agent', { exact: true });
  await expect(prompt).toHaveValue(/^Map this repository/);
  const draft = await prompt.inputValue();
  // Choosing again replaces the draft rather than appending to it.
  await page.getByRole('button', { name: 'Map the architecture', exact: true }).click();
  await expect(prompt).toHaveValue(draft);
  await page.getByRole('button', { name: 'Explain a flow', exact: true }).click();
  await expect(prompt).toHaveValue(/^Help me choose a critical flow/);
  await page.getByRole('button', { name: 'Map the architecture', exact: true }).click();
  await expect(prompt).toHaveValue(draft);
  const sent = () =>
    page.evaluate(() =>
      (window as any).testDesktop.calls.filter((c: any) => c.command === 'agent_prompt'),
    );
  expect(await sent()).toHaveLength(0);
  await page.evaluate(() => {
    (window as any).testDesktop.failPrompt = true;
  });
  await page.getByRole('button', { name: 'Send message' }).click();
  await expect(page.locator('.agent-panel')).toBeVisible();
  await expect(prompt).toHaveValue(draft);
  await page.evaluate(() => {
    (window as any).testDesktop.failPrompt = false;
  });
  await page.getByRole('button', { name: 'Send message' }).click();
  await expect(prompt).toHaveValue('');
  await expect.poll(sent).toHaveLength(2);
  expect((await sent())[1].args.text).toBe(draft);
  // Sent from the home composer, it starts a new conversation instead of continuing the last one.
  const calls = await page.evaluate(() =>
    (window as any).testDesktop.calls.filter((c: any) =>
      ['agent_new_conversation', 'agent_prompt'].includes(c.command),
    ),
  );
  // The retry reuses the conversation the failed send emptied.
  expect(calls.map((c: any) => c.command)).toEqual([
    'agent_new_conversation',
    'agent_prompt',
    'agent_prompt',
  ]);
});

test('installed agents connect directly from chat and preserve the unsent draft', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page, { connected: false });
  await page.goto('/');
  await openProject(page);
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  const panel = page.getByRole('complementary', { name: 'Local coding agent' });
  await panel.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await expect(panel.getByRole('button', { name: 'Use Codex', exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Use Gemini CLI', exact: true })).toBeVisible();
  await expect(panel.getByRole('button', { name: 'Use Claude Code', exact: true })).toBeEnabled();
  await expect(panel.getByRole('button', { name: 'Set up', exact: true })).toHaveCount(0);
  await expect(panel.getByLabel('Executable', { exact: true })).toHaveCount(0);
  await expect(panel.getByText('Working directory', { exact: true })).toHaveCount(0);
  const composer = panel.getByLabel('Message your agent');
  await composer.fill('Explain the checkout flow.');
  await panel.getByRole('button', { name: 'Choose agent', exact: true }).click();
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
  await openProject(page);
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  await page.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await page.evaluate(() => ((window as any).testDesktop.failConnect = true));
  await page.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Agent could not connect');
  await page.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Use Codex', exact: true })).toBeEnabled();
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.failConnect = false;
    desktop.needsAuth = true;
  });
  await page.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Codex',
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
  await openProject(page);
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  await page.getByRole('button', { name: 'Choose agent', exact: true }).click();
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
      executable: '/tmp/codex',
      available: true,
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
  await openProject(page);
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  await page.getByLabel('Message your agent').fill('Keep this draft');
  await page.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await page.evaluate(() => ((window as any).testDesktop.deferConnect = true));
  await page.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Choose agent', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await page.getByRole('button', { name: /Ask your agent/ }).click();
  await page.evaluate(() => (window as any).testDesktop.finishConnect());
  await expect(page.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Codex',
  );
  await expect(page.getByLabel('Message your agent')).toHaveValue('Keep this draft');
  await expect(page.getByRole('button', { name: 'Send message' })).toBeEnabled();
  expect(errors).toEqual([]);
});

test('project and side chat share composer choices, brand marks and the unsent draft', async ({
  page,
}) => {
  await mockDesktop(page, { connected: false });
  await page.goto('/');
  await openProject(page);
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  const chat = page.getByRole('complementary', { name: 'Local coding agent' });
  const composer = chat.locator('form');
  await composer.getByLabel('Message your agent').fill('Explain this repository');
  await expect(composer.getByRole('button', { name: 'Choose model' })).toBeDisabled();
  await composer.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await expect(
    composer.getByRole('button', { name: 'Use Codex', exact: true }).locator('svg'),
  ).toHaveCount(1);
  await composer.getByRole('button', { name: 'Use Codex', exact: true }).click();
  await composer.getByRole('button', { name: 'Choose model' }).click();
  await composer.getByRole('button', { name: 'Deep model', exact: true }).click();
  await expect(composer.getByRole('button', { name: 'Choose model' })).toContainText('Deep model');
  await page.screenshot({ path: test.info().outputPath('project-chat-composer.png') });
  await page.getByRole('button', { name: 'Ask your agent' }).click();
  await expect(chat).toHaveCount(1);
  await expect(chat.getByLabel('Message your agent')).toHaveValue('Explain this repository');
  await expect(chat.getByRole('button', { name: 'Choose model' })).toContainText('Deep model');
  await chat.getByRole('button', { name: 'Choose agent', exact: true }).click();
  await chat.getByRole('button', { name: 'Use Claude Code', exact: true }).click();
  await expect(chat.getByRole('button', { name: 'Choose model' })).toContainText('Fast model');
  await chat.getByRole('button', { name: 'Choose model' }).click();
  await chat.getByRole('button', { name: 'Deep model', exact: true }).click();
  const calls = await page.evaluate(() => (window as any).testDesktop.calls);
  expect(
    calls.filter((c: any) => c.command === 'agent_set_model').map((c: any) => c.args.modelId),
  ).toEqual(['codex-deep', 'claude-deep']);
  expect(calls.some((c: any) => c.command === 'agent_prompt')).toBe(false);
  await chat.getByRole('button', { name: 'Close agent panel' }).click();
  await expect(chat.getByRole('button', { name: 'Choose agent', exact: true })).toContainText(
    'Claude Code',
  );
  await expect(chat.getByRole('button', { name: 'Choose model' })).toContainText('Deep model');
  await chat.getByRole('button', { name: 'Choose model' }).click();
  await chat.getByRole('button', { name: 'Send message' }).focus();
  await expect(chat.getByRole('region', { name: 'Available models' })).toHaveCount(0);
  await chat.getByRole('button', { name: 'Send message' }).click();
  await expect(chat.getByLabel('Message your agent')).toHaveValue('');
  const sent = await page.evaluate(() =>
    (window as any).testDesktop.calls.filter((c: any) => c.command === 'agent_prompt'),
  );
  expect(sent).toEqual([
    { command: 'agent_prompt', args: { projectId: sample.id, text: 'Explain this repository' } },
  ]);
});

test('overview starts fresh chats in the side panel and retains older conversations to reopen', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  const history = page.getByRole('region', { name: 'Conversations', exact: true });
  await expect(
    history.getByRole('button', { name: 'Open Map checkout flow with Fixture ACP agent' }),
  ).toBeVisible();
  await expect(page.getByRole('log', { name: 'Agent conversation' })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Commerce domain' })).toHaveCount(0);
  await page.getByLabel('Message your agent').fill('Start a separate discussion');
  await page.getByRole('button', { name: 'Send message' }).click();
  const panel = page.locator('.agent-panel');
  await expect(panel).toBeVisible();
  await expect(panel.getByRole('log', { name: 'Agent conversation' })).toContainText(
    'Start a separate discussion',
  );
  const calls = await page.evaluate(() =>
    (window as any).testDesktop.calls.filter((c: any) =>
      ['agent_new_conversation', 'agent_prompt'].includes(c.command),
    ),
  );
  expect(calls.map((c: any) => c.command)).toEqual(['agent_new_conversation', 'agent_prompt']);
  await panel.getByRole('button', { name: 'Close agent panel' }).click();
  await expect(page.getByRole('log', { name: 'Agent conversation' })).toHaveCount(0);
  await expect(history.getByRole('button')).toHaveCount(2);
  await expect(history.getByText('Current', { exact: true })).toHaveCount(1);
  await expect(
    history.getByRole('button', { name: 'Open Map checkout flow with Fixture ACP agent' }),
  ).toContainText('Reopen');
  await page.screenshot({ path: test.info().outputPath('overview-conversations.png') });
  await history
    .getByRole('button', { name: 'Open Map checkout flow with Fixture ACP agent' })
    .click();
  await expect(panel).toBeVisible();
  await expect(panel.getByRole('log')).toContainText('Reopened Map checkout flow.');
  await panel.getByLabel('Message your agent').fill('Keep my draft');
  await panel.getByRole('button', { name: 'New conversation' }).click();
  await expect(panel.getByLabel('Message your agent')).toHaveValue('Keep my draft');
  await panel.getByRole('button', { name: 'Close agent panel' }).click();
  await expect(history.getByRole('button')).toHaveCount(2);
  await expect(history.getByText('Current', { exact: true })).toHaveCount(0);
  await page.evaluate(() => ((window as any).testDesktop.failReopen = true));
  await history
    .getByRole('button', { name: 'Open Map checkout flow with Fixture ACP agent' })
    .click();
  await expect(history.getByRole('alert')).toContainText('Stop the current turn');
  await expect(panel).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('overview reflects agent-owned session renames without creating another row', async ({
  page,
}) => {
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  const history = page.getByRole('region', { name: 'Conversations', exact: true });
  await expect(
    history.getByRole('button', { name: 'Open Map checkout flow with Fixture ACP agent' }),
  ).toBeVisible();
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.conversations[0].label = 'Review payment boundaries';
    desktop.emit('agent:update', structuredClone(desktop.snapshot));
  });
  await expect(history.getByRole('button')).toHaveCount(1);
  await history
    .getByRole('button', { name: 'Open Review payment boundaries with Fixture ACP agent' })
    .click();
  await expect(page.locator('.agent-panel').getByRole('log')).toContainText(
    'Reopened Review payment boundaries.',
  );
});

test('an agent can open a diagram in the window and select one of its nodes', async ({ page }) => {
  await mockDesktop(page);
  await page.goto('/');
  await openProject(page);
  const navigate = (request: object) =>
    page.evaluate((request) => {
      const desktop = (window as any).testDesktop;
      desktop.emit('atlas:navigate', { projectId: desktop.project.id, ...request });
    }, request);
  await navigate({ kind: 'diagram', id: '.schematlas/diagrams/architecture.mmd', node: 'db' });
  const canvas = page.getByLabel('Interactive diagram', { exact: true });
  const db = canvas.locator('[data-node-id="db"]');
  await expect(db).toHaveAttribute('data-selected', /.*/);
  await expect(page.getByLabel('Diagram title')).toHaveValue('Northstar architecture');
  // Requests for another project are not this window's to act on.
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await page.evaluate(() =>
    (window as any).testDesktop.emit('atlas:navigate', {
      projectId: 'someone-else',
      kind: 'diagram',
      id: '.schematlas/diagrams/architecture.mmd',
    }),
  );
  await expect(page.getByRole('region', { name: 'Ask your agent', exact: true })).toBeVisible();
  await expect(canvas).toHaveCount(0);
});

test('sequence diagrams are drawn as lifelines, and participants open their details', async ({
  page,
}) => {
  await mockDesktop(page);
  const text =
    'sequenceDiagram\n  participant W as Web\n  participant A as API\n  W->>A: POST /orders\n  A-->>W: 201 Created\n';
  const diagram = {
    kind: 'sequence',
    nodes: [
      { id: 'W', label: 'Web' },
      { id: 'A', label: 'API' },
    ],
    edges: [
      { from: 'W', to: 'A', label: 'POST /orders', stroke: 'normal', head: 'arrow', length: 1 },
      { from: 'A', to: 'W', label: '201 Created', stroke: 'dotted', head: 'arrow', length: 1 },
    ],
    sequence: {
      steps: [
        { step: 'message', from: 'W', to: 'A', label: 'POST /orders', arrow: 'filled' },
        {
          step: 'message',
          from: 'A',
          to: 'W',
          label: '201 Created',
          dotted: true,
          arrow: 'filled',
        },
      ],
    },
  };
  await page.addInitScript(
    ({ diagram, text }) => {
      const core = (window as any).__TAURI__.core;
      const invoke = core.invoke;
      core.invoke = async (command: string, args: any) =>
        command === 'get_diagram' && args.id === 'docs/checkout.mmd'
          ? {
              id: args.id,
              path: args.id,
              format: 'mermaid',
              title: 'Checkout',
              hash: 'h',
              editable: 'annotations',
              source: text,
              text,
              diagram,
            }
          : invoke(command, args);
    },
    { diagram, text },
  );
  await page.goto('/');
  await openProject(page);
  await page.evaluate(() => {
    const desktop = (window as any).testDesktop;
    desktop.emit('atlas:navigate', {
      projectId: desktop.project.id,
      kind: 'diagram',
      id: 'docs/checkout.mmd',
    });
  });
  const view = page.getByRole('region', { name: 'Sequence diagram' });
  await expect(view).toBeVisible();
  // Participants head and foot their lifelines; messages read top to bottom.
  await expect(view.locator('[data-node-id="A"]')).toHaveCount(2);
  await expect(view.locator('.sequence-message text').first()).toContainText('POST /orders');
  await expect(view.locator('.sequence-message path[stroke-dasharray]')).toHaveCount(1);
  await expect(page.getByLabel('Interactive diagram', { exact: true })).toHaveCount(0);
  await view.locator('[data-node-id="A"]').first().click();
  const details = page.getByRole('complementary', { name: 'Node details' });
  await expect(details.getByRole('heading', { name: 'API' })).toBeVisible();
  await expect(view.locator('[data-node-id="A"]').first()).toHaveAttribute('aria-pressed', 'true');
});
