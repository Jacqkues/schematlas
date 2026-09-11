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

test('middle-button dragging pans over cards and groups without changing the layout or selection', async ({
  page,
}) => {
  await demo(page);
  await card(page, 'customers').click();
  const inspect = page.getByRole('button', { name: 'Inspect main.customers' });
  await expect(inspect).toBeVisible();
  const before = await positions(page);
  const canvas = page.getByLabel('Interactive schema graph', { exact: true });
  const transform = () =>
    canvas
      .locator(':scope > div')
      .first()
      .evaluate((el) => new DOMMatrix(getComputedStyle(el).transform).toString());
  for (const target of [
    card(page, 'customers'),
    page.getByRole('button', { name: /^Move group / }).first(),
  ]) {
    const box = await target.boundingBox();
    if (!box) throw new Error('Missing pan target');
    const original = await transform();
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down({ button: 'middle' });
    await page.mouse.move(box.x + box.width / 2 + 65, box.y + box.height / 2 + 35, { steps: 12 });
    await page.mouse.up({ button: 'middle' });
    await expect.poll(transform).not.toBe(original);
    const delta = await canvas
      .locator(':scope > div')
      .first()
      .evaluate((el, original) => {
        const before = new DOMMatrix(original),
          after = new DOMMatrix(getComputedStyle(el).transform);
        return [after.e - before.e, after.f - before.f, after.a - before.a];
      }, original);
    expect(delta[0]).toBeCloseTo(65, 0);
    expect(delta[1]).toBeCloseTo(35, 0);
    expect(delta[2]).toBe(0);
    expect(await positions(page)).toEqual(before);
    await expect(inspect).toBeVisible();
  }
  await card(page, 'customers').click({ button: 'middle' });
  await expect(inspect).toBeVisible();
  await expect(page.getByRole('button', { name: 'Close inspector' })).toHaveCount(0);
});

test('drag frames preserve edges and group controls, flush the final position and roll back cancellation', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  const canvas = page.getByLabel('Interactive schema graph', { exact: true });
  await expect(canvas.locator(':scope > div').first()).not.toHaveClass(/transition-transform/);
  const zoom = await canvas
    .locator(':scope > div')
    .first()
    .evaluate((el) => new DOMMatrix((el as HTMLElement).style.transform).a);
  const before = await positions(page);
  const node = card(page, 'customers');
  const nodeBounds = (await node.boundingBox())!;
  const box = { x: Math.round(nodeBounds.x), y: Math.round(nodeBounds.y) };
  await page
    .locator('.edge-path')
    .evaluateAll((edges) => edges.forEach((edge) => edge.setAttribute('data-retained', 'yes')));
  const group = page.getByRole('button', { name: /^Move group / }).first();
  await group.evaluate((el) => el.setAttribute('data-retained', 'yes'));
  // Several input events arrive before the next frame. Pointer-up must save the last one.
  await node.dispatchEvent('pointerdown', {
    button: 0,
    pointerId: 9,
    clientX: box.x,
    clientY: box.y,
  });
  await canvas.evaluate((el, { x, y }) => {
    for (const dx of [10, 25, 60])
      el.dispatchEvent(
        new PointerEvent('pointermove', {
          bubbles: true,
          pointerId: 9,
          clientX: x + dx,
          clientY: y + 30,
        }),
      );
    el.dispatchEvent(
      new PointerEvent('pointerup', {
        bubbles: true,
        pointerId: 9,
        clientX: x + 60,
        clientY: y + 30,
      }),
    );
  }, box);
  await expect.poll(() => positions(page)).not.toEqual(before);
  const after = await positions(page);
  const id = sample.sources[0].graph.entities.find((e) => e.name === 'customers')!.id;
  expect((after[id].x - before[id].x) * zoom).toBeCloseTo(60, 1);
  expect((after[id].y - before[id].y) * zoom).toBeCloseTo(30, 1);
  await expect(page.locator('.edge-path:not([data-retained])')).toHaveCount(0);
  await expect(group).toHaveAttribute('data-retained', 'yes');
  const style = await node.locator('..').getAttribute('style');
  await node.dispatchEvent('pointerdown', { button: 0, pointerId: 10, clientX: 400, clientY: 400 });
  await canvas.dispatchEvent('pointermove', { pointerId: 10, clientX: 470, clientY: 460 });
  await expect(node.locator('..')).not.toHaveAttribute('style', style!);
  await canvas.dispatchEvent('pointercancel', { pointerId: 10 });
  await expect(node.locator('..')).toHaveAttribute('style', style!);
  expect(await positions(page)).toEqual(after);
  // A queued frame is safely cancelled when its canvas is disposed.
  await node.dispatchEvent('pointerdown', { button: 0, pointerId: 11, clientX: 400, clientY: 400 });
  await canvas.dispatchEvent('pointermove', { pointerId: 11, clientX: 500, clientY: 500 });
  await page.getByRole('button', { name: /Commerce API/ }).click();
  await expect(cards(page)).toHaveCount(9);
  expect(errors).toEqual([]);
});

test('inspector text settles without a retained animation transform', async ({ page }) => {
  await demo(page);
  await card(page, 'customers').click();
  await page.getByRole('button', { name: 'Inspect main.customers' }).click();
  const inspector = page.locator('.inspector');
  await expect(inspector).toHaveCSS('transform', 'none');
  await expect(inspector).toHaveCSS('animation-name', 'none');
  await expect(inspector.locator('strong').first()).toHaveCSS('font-size', '12px');
  await expect(card(page, 'customers').locator('..')).toHaveCSS('will-change', 'auto');
});
