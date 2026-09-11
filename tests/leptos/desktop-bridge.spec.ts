import { test, expect, type Page } from '@playwright/test';
import sample from '../../ui/src/sample.json' with { type: 'json' };

async function mockDesktop(page: Page) {
  await page.addInitScript((project) => {
    const listeners = new Map<string, Set<(event: { payload: unknown }) => void>>();
    const calls: { command: string; args: any }[] = [];
    const snapshot = {
      projectId: project.id,
      status: 'ready',
      agentName: 'Fixture ACP agent',
      messages: [
        {
          id: 'answer',
          role: 'assistant',
          text: '# Commerce domain\n\n**Customers** own orders.\n\n```sql\nSELECT 1;\n```\n\n<script>alert(1)</script>',
        },
      ],
      reviews: [],
    };
    Object.assign(window, {
      testDesktop: {
        calls,
        project,
        snapshot,
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
              case 'agent_status':
                return snapshot;
              case 'discover_agents':
                return [
                  {
                    name: 'Fixture ACP adapter',
                    executable: '/tmp/fixture-acp',
                    args: [],
                    acpReady: true,
                  },
                ];
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
  }, sample);
}

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
