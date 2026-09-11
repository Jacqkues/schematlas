import { test, expect, type Page } from '@playwright/test';
import sample from '../../ui/src/sample.json' with { type: 'json' };

const storageKey = 'schema-atlas-browser-preview-v1';
const cards = (page: Page) => page.locator('.entity-node');
const card = (page: Page, name: string) =>
  cards(page).filter({ has: page.locator('strong', { hasText: new RegExp(`^${name}$`) }) });

async function demo(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await expect(cards(page)).toHaveCount(6);
}

async function positions(page: Page) {
  return page.evaluate(
    (key) => JSON.parse(localStorage.getItem(key)!)[0].sources[0].positions,
    storageKey,
  );
}

test('selection, inspector, source switching, search and light theme preserve the graph', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  await card(page, 'customers').click();
  await expect(page.getByRole('button', { name: 'Close inspector' })).toHaveCount(0);
  await expect(page.locator('.edge-path[data-state="active"]')).not.toHaveCount(0);
  await expect(page.locator('.entity-node[data-state="dimmed"]')).not.toHaveCount(0);
  await page.getByRole('button', { name: 'Inspect main.customers' }).click();
  await page.getByRole('button', { name: 'Close inspector' }).click();
  const before = await positions(page);
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await expect(cards(page).first()).toHaveCSS('background-color', 'rgb(255, 255, 255)');
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  expect(await positions(page)).toEqual(before);
  await page.getByRole('button', { name: /Commerce API/ }).click();
  await expect(cards(page)).toHaveCount(9);
  await page.getByRole('button', { name: /Commerce database/ }).click();
  await expect(cards(page)).toHaveCount(6);
  await page.getByRole('searchbox', { name: 'Search schema' }).fill('customers');
  await expect(cards(page)).toHaveCount(1);
  await page.getByRole('searchbox', { name: 'Search schema' }).fill('');
  await expect(cards(page)).toHaveCount(6);
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(cards(page)).toHaveCount(6);
  expect(errors).toEqual([]);
});

test('named colored groups drag together, persist and undo', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  await page.getByRole('button', { name: 'Groups', exact: true }).click();
  await page.getByRole('button', { name: 'New group', exact: true }).click();
  await page.getByLabel('Group name').fill('Catalog');
  await page.getByRole('button', { name: 'Blue group color' }).click();
  await page.getByRole('checkbox', { name: /products/ }).check();
  await page.getByRole('button', { name: 'Create group', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await page.getByRole('button', { name: 'Close map options' }).click();
  const group = page.getByRole('button', { name: 'Move group Catalog', exact: true });
  await expect(group).toBeVisible();
  const before = await positions(page);
  const box = await group.boundingBox();
  if (!box) throw new Error('Missing group bounds');
  await page.mouse.move(box.x + 80, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + 130, box.y + box.height / 2 - 30, { steps: 8 });
  await page.mouse.up();
  await expect.poll(() => positions(page)).not.toEqual(before);
  const after = await positions(page);
  const productId = sample.sources[0].graph.entities.find((e) => e.name === 'products')!.id;
  for (const [id, pos] of Object.entries(before)) {
    if (id !== productId) expect(after[id]).toEqual(pos);
  }
  await page.reload();
  await expect(group).toBeVisible();
  expect(await positions(page)).toEqual(after);
  await page.getByRole('button', { name: 'Groups', exact: true }).click();
  await page.getByRole('button', { name: 'Undo canvas edit' }).click();
  await expect.poll(() => positions(page)).toEqual(before);
  expect(errors).toEqual([]);
});

test('multiple schemas filter independently and keep explicitly included related tables', async ({
  page,
}) => {
  const project = structuredClone(sample);
  project.sources[0].graph.entities.forEach((e) => {
    e.namespace = e.name === 'products' ? 'catalog' : 'commerce';
  });
  await page.addInitScript(
    ({ key, project }) => localStorage.setItem(key, JSON.stringify([project])),
    { key: storageKey, project },
  );
  await page.goto('/');
  await expect(cards(page)).toHaveCount(6);
  await page.getByRole('button', { name: 'Schemas', exact: true }).click();
  await page.getByRole('button', { name: /catalog/ }).click();
  await page.getByRole('checkbox', { name: /linked schemas/i }).uncheck();
  await expect(cards(page)).toHaveCount(1);
  await page.getByRole('checkbox', { name: /linked schemas/i }).check();
  await expect(cards(page)).toHaveCount(2);
});

test('theme switching works without preference storage', async ({ page }) => {
  await page.addInitScript(() => {
    const setItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (key === 'schematlas.theme') throw new DOMException('Storage unavailable');
      return setItem.call(this, key, value);
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Switch to light theme' }).click();
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  await page.getByRole('button', { name: 'Switch to dark theme' }).click();
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'dark');
});

test('automatic layout runs in a worker and persists a complete arrangement', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  const before = await positions(page);
  const worker = page.waitForEvent('worker');
  await page.getByRole('button', { name: 'Auto layout', exact: true }).click();
  expect((await worker).url()).toContain('layout-worker-loader.js');
  await expect(page.getByRole('button', { name: 'Auto layout', exact: true })).toBeEnabled();
  await expect.poll(() => positions(page)).not.toEqual(before);
  expect(Object.keys(await positions(page))).toHaveLength(6);
  await expect(cards(page)).toHaveCount(6);
  expect(errors).toEqual([]);
});
