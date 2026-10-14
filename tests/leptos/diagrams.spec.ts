import { test, expect, type Page } from '@playwright/test';

const atlasKey = 'schema-atlas-browser-preview-atlas-v2';
const sample = 'docs/architecture.mmd';
const canvas = (page: Page) => page.getByLabel('Interactive diagram', { exact: true });
const nodes = (page: Page) => canvas(page).locator('.diagram-node');
const node = (page: Page, label: string) => nodes(page).filter({ hasText: label });

async function openArchitecture(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await page.getByRole('button', { name: 'Northstar architecture' }).first().click();
  await expect(nodes(page)).toHaveCount(5);
}
async function stored(page: Page) {
  return page.evaluate((key) => {
    const all = JSON.parse(localStorage.getItem(key)!);
    return Object.values(all)[0] as any;
  }, atlasKey);
}

test('diagrams are laid out, edited in place and saved to the repository atlas', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openArchitecture(page);
  await expect(page.locator('.diagram-edge')).toHaveCount(6);
  await expect(page.getByText('POST /orders')).toBeVisible();
  // The worker arranged the unplaced diagram and the layout was written back.
  await expect
    .poll(async () => Object.keys((await stored(page)).diagrams[sample].layout ?? {}).length)
    .toBe(5);

  // Double-click adds a node and starts editing its label in place.
  const box = (await canvas(page).boundingBox())!;
  await page.mouse.dblclick(box.x + 40, box.y + box.height - 90);
  await expect(nodes(page)).toHaveCount(6);
  await page.getByLabel('Node label').fill('Search index');
  await page.getByLabel('Node label').press('Enter');
  await expect(node(page, 'Search index')).toHaveCount(1);
  // Details open on request, from the floating toolbar of the selected node.
  await page.getByRole('button', { name: 'Details', exact: true }).click();
  await page.getByLabel('Add a code reference').fill('src/search/index.rs:4-20');
  await page.getByRole('button', { name: 'Add reference' }).click();
  await expect(page.getByRole('button', { name: 'src/search/index.rs:4-20' })).toBeVisible();
  await expect
    .poll(async () => (await stored(page)).diagrams[sample].nodes.map((n: any) => n.label))
    .toContain('Search index');

  // Connect mode still works from the keyboard, wherever the card sits on screen.
  await page.getByRole('button', { name: 'Close node details' }).click();
  await page.getByRole('button', { name: 'Connect nodes' }).click();
  await node(page, 'Orders API').press('Enter');
  await node(page, 'Search index').press('Enter');
  await expect(page.locator('.diagram-edge')).toHaveCount(7);
  await page.keyboard.press('Escape');

  // Undo and redo walk whole gestures.
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.locator('.diagram-edge')).toHaveCount(6);
  await page.getByRole('button', { name: 'Redo' }).click();
  await expect(page.locator('.diagram-edge')).toHaveCount(7);

  await page.reload();
  await page.getByRole('button', { name: 'Northstar architecture' }).first().click();
  await expect(nodes(page)).toHaveCount(6);
  expect(errors).toEqual([]);
});

test('right-click, handle drags, selection and labels feel like a whiteboard', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await openArchitecture(page);
  const box = (await canvas(page).boundingBox())!;
  const empty = await canvas(page).evaluate((element) => {
    const bounds = element.getBoundingClientRect();
    const cards = [...element.querySelectorAll('.diagram-node')].map((node) =>
      node.getBoundingClientRect(),
    );
    for (let x = bounds.right - 60; x > bounds.left + 60; x -= 80) {
      for (let y = bounds.top + 100; y < bounds.bottom - 70; y += 80) {
        if (
          cards.every(
            (r) => x < r.left - 20 || x > r.right + 20 || y < r.top - 20 || y > r.bottom + 20,
          )
        ) {
          return { x, y };
        }
      }
    }
    throw new Error('No empty canvas point is available');
  });

  // Right-click empty canvas: pick a kind, then type the name straight away.
  await page.mouse.click(empty.x, empty.y, { button: 'right' });
  const menu = page.getByRole('menu', { name: 'Canvas actions' });
  await expect(menu).toBeVisible();
  await menu.getByRole('menuitem', { name: 'Data store' }).click();
  await expect(menu).toHaveCount(0);
  await page.keyboard.type('Cache');
  await page.keyboard.press('Enter');
  await expect(node(page, 'Cache')).toHaveCount(1);
  await expect
    .poll(
      async () =>
        (await stored(page)).diagrams[sample].nodes.find((n: any) => n.label === 'Cache')?.shape,
    )
    .toBe('cylinder');

  // Drag from a node's handle onto another node to connect them.
  const drag = async (from: { x: number; y: number }, to: { x: number; y: number }) => {
    await page.mouse.move(from.x, from.y);
    await page.mouse.down();
    await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
    await page.mouse.move(to.x, to.y, { steps: 4 });
    await page.mouse.up();
  };
  const center = async (label: string) => {
    const b = (await node(page, label).boundingBox())!;
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  const handle = async (label: string, side: string) => {
    await node(page, label).hover();
    const b = (await node(page, label).locator(`[data-handle="${side}"]`).boundingBox())!;
    return { x: b.x + b.width / 2, y: b.y + b.height / 2 };
  };
  // Shift+1 zooms to fit, bringing every card back on screen.
  await page.keyboard.press('Shift+Digit1');
  await page.waitForTimeout(300);
  await drag(await handle('Orders API', 'bottom'), await center('Cache'));
  await expect(page.locator('.diagram-edge')).toHaveCount(7);

  // Dropping a connection on empty canvas creates a connected node to name.
  // Find empty canvas: beside the drawing, wherever the fit placed it.
  const cards = await nodes(page).evaluateAll((els) =>
    els.map((el) => {
      const r = el.getBoundingClientRect();
      return { left: r.left, right: r.right, top: r.top, bottom: r.bottom };
    }),
  );
  const right = Math.max(...cards.map((c) => c.right));
  const left = Math.min(...cards.map((c) => c.left));
  const top = Math.min(...cards.map((c) => c.top));
  const bottom = Math.max(...cards.map((c) => c.bottom));
  const spot =
    right + 150 < box.x + box.width
      ? { x: right + 110, y: (top + bottom) / 2 }
      : left - 150 > box.x
        ? { x: left - 110, y: (top + bottom) / 2 }
        : { x: (left + right) / 2, y: Math.min(bottom + 90, box.y + box.height - 70) };
  await drag(await handle('Cache', 'bottom'), spot);
  await expect(nodes(page)).toHaveCount(7);
  await page.getByLabel('Node label').fill('Redis');
  await page.getByLabel('Node label').press('Enter');
  await expect(page.locator('.diagram-edge')).toHaveCount(8);

  // Right-click a node to recolor it.
  const cache = await center('Cache');
  await page.mouse.click(cache.x, cache.y, { button: 'right' });
  await page.getByRole('button', { name: 'Color #e0795f' }).click();
  await expect
    .poll(
      async () =>
        (await stored(page)).diagrams[sample].nodes.find((n: any) => n.label === 'Cache').annotation
          ?.extra?.color,
    )
    .toBe('#e0795f');

  // Select everything, then clear with Escape.
  await page.keyboard.press('ControlOrMeta+a');
  await expect(canvas(page).locator('[aria-pressed="true"].diagram-node')).toHaveCount(7);
  await page.keyboard.press('Escape');
  await expect(canvas(page).locator('[aria-pressed="true"].diagram-node')).toHaveCount(0);

  // Edge labels edit in place.
  await page.keyboard.press('Shift+Digit1');
  await page.waitForTimeout(300);
  await page.getByText('POST /orders').dblclick();
  await page.getByLabel('Connection label').fill('HTTPS');
  await page.getByLabel('Connection label').press('Enter');
  await expect(page.getByText('HTTPS', { exact: true })).toBeVisible();

  // Scrolling pans without zooming; Ctrl+scroll zooms.
  const zoom = page.getByRole('button', { name: 'Reset zoom' });
  const before = await zoom.textContent();
  const layer = canvas(page).locator(':scope > div').nth(0);
  const transform = await layer.getAttribute('style');
  await page.mouse.move(empty.x, empty.y);
  await page.mouse.wheel(0, 120);
  await expect(zoom).toHaveText(before!);
  expect(await layer.getAttribute('style')).not.toEqual(transform);
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, -100);
  await page.keyboard.up('Control');
  await expect(zoom).not.toHaveText(before!);
  expect(errors).toEqual([]);
});

test('docs render Markdown, list code references and edit with compare-and-swap', async ({
  page,
}) => {
  await openArchitecture(page);
  await page.getByRole('button', { name: 'Northstar Commerce' }).first().click();
  await expect(page.locator('article.markdown h1')).toHaveText('Northstar Commerce');
  await expect(page.getByRole('button', { name: 'src/api/checkout.rs:18-64' })).toBeVisible();
  await page.getByRole('button', { name: 'docs/architecture.mmd', exact: true }).click();
  await expect(nodes(page)).toHaveCount(5);
  await page.getByRole('button', { name: 'Northstar Commerce' }).first().click();
  await page.getByRole('button', { name: 'Edit', exact: true }).click();
  await page.getByLabel('Markdown').fill('# Northstar Commerce\n\nUpdated.');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('article.markdown p')).toHaveText('Updated.');
  expect((await stored(page)).docs.overview).toContain('Updated.');
});

test('the overview creates diagrams and docs, and team sync stays desktop-only', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'New project' }).click();
  await page.getByLabel('Project name').fill('Atlas');
  await page.getByRole('button', { name: 'Create project' }).click();
  await expect(
    page.getByRole('heading', { name: 'What would you like to understand?' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'New diagram' }).first().click();
  await page.getByLabel('Title').fill('Payments flow');
  await page.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(page.getByLabel('Diagram title')).toHaveValue('Payments flow');
  await expect(page.getByText('An empty diagram')).toBeVisible();
  await page.getByRole('button', { name: 'Add node' }).click();
  await expect(nodes(page)).toHaveCount(1);
  await page.getByRole('button', { name: /Team/ }).click();
  await expect(page.getByText('Team sync runs in the desktop app.')).toBeVisible();
});

test('component search opens an explanation and follows its evidence and connections', async ({
  page,
}) => {
  await openArchitecture(page);
  await page.getByRole('searchbox', { name: 'Search nodes' }).fill('prices orders');
  await page.getByLabel('Matching components').getByRole('button', { name: 'Orders API' }).click();
  const details = page.getByRole('complementary', { name: 'Node details' });
  await expect(details.getByRole('heading', { name: 'Orders API' })).toBeVisible();
  await expect(
    details.getByText('Validates carts, prices orders and coordinates payment.'),
  ).toBeVisible();
  await expect(
    details.getByRole('button', { name: 'src/api/checkout.rs:18-64', exact: true }),
  ).toBeVisible();
  await expect(details.getByLabel('Label', { exact: true })).not.toBeVisible();
  await details.getByRole('button', { name: 'Go to Commerce DB', exact: true }).click();
  await expect(details.getByRole('heading', { name: 'Commerce DB' })).toBeVisible();
  await expect(node(page, 'Commerce DB')).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Close node details' }).click();
  await page.getByRole('searchbox', { name: 'Search nodes' }).fill('no-such-component');
  await expect(page.getByText('No matching components')).toBeVisible();
  await page.getByRole('searchbox', { name: 'Search nodes' }).press('Escape');
  await expect(page.getByLabel('Matching components')).toHaveCount(0);
});

test('project prompts remain drafts until sent and survive closing the agent panel', async ({
  page,
}) => {
  await openArchitecture(page);
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await page
    .getByLabel('Message your agent', { exact: true })
    .fill('Explain the checkout flow and cite its code.');
  await page.getByRole('button', { name: 'Ask your agent' }).click();
  await expect(page.getByLabel('Message your agent')).toHaveValue(
    'Explain the checkout flow and cite its code.',
  );
  await expect(page.getByText('Browser preview · Desktop agents only')).toBeVisible();
  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await page.getByRole('button', { name: 'Ask your agent' }).click();
  await expect(page.getByLabel('Message your agent')).toHaveValue(
    'Explain the checkout flow and cite its code.',
  );
  const persisted = await page.evaluate(() => Object.values(localStorage).join(' '));
  expect(persisted).not.toContain('Explain the checkout flow and cite its code.');
  await page.getByRole('button', { name: 'Close agent panel' }).click();
  await page.getByRole('button', { name: 'New project', exact: true }).click();
  await page.getByLabel('Project name').fill('Separate project');
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await page.getByRole('button', { name: 'Ask your agent' }).click();
  await expect(page.getByLabel('Message your agent')).toHaveValue('');
});

test('collapsing navigation frees the canvas without resetting the map', async ({ page }) => {
  await openArchitecture(page);
  const before = (await canvas(page).boundingBox())!;
  await node(page, 'Storefront').press('Enter');
  await page.getByRole('button', { name: 'Collapse sidebar' }).click();
  await expect(node(page, 'Storefront')).toHaveAttribute('aria-pressed', 'true');
  const after = (await canvas(page).boundingBox())!;
  expect(after.width).toBeGreaterThan(before.width + 150);
  await page.getByRole('button', { name: 'Expand sidebar' }).click();
  await expect(
    page.getByRole('button', { name: 'Northstar architecture', exact: true }),
  ).toBeVisible();
});
