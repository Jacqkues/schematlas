import { test, expect, type Page } from '@playwright/test';
import sample from '../../ui/src/sample.json' with { type: 'json' };
import type { Project } from '../../src/lib/types';

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
  // Tables are buttons too now, so scope the chip lookup to the schema panel.
  await page
    .locator('#map-options')
    .getByRole('button', { name: /catalog/ })
    .click();
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

test('relationship ports and cardinality follow table placement while dragging', async ({
  page,
}) => {
  const project = structuredClone(sample) as Project;
  const source = project.sources[0];
  const orders = source.graph.entities.find((e) => e.name === 'orders')!;
  const customers = source.graph.entities.find((e) => e.name === 'customers')!;
  const relation = source.graph.relations.find(
    (r) => r.source === orders.id && r.target === customers.id,
  )!;
  source.graph.entities = [orders, customers];
  source.graph.relations = [relation];
  source.positions = {
    [orders.id]: { x: 650, y: 100 },
    [customers.id]: { x: 50, y: 100 },
  };
  source.groups = [];
  project.sources = [source];
  await page.addInitScript(
    ({ key, project }) => {
      if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify([project]));
    },
    { key: storageKey, project },
  );
  await page.goto('/');
  await expect(cards(page)).toHaveCount(2);
  const canvas = page.getByLabel('Interactive schema graph', { exact: true });
  const viewport = canvas.locator(':scope > div').first();
  await expect(viewport).not.toHaveClass(/transition-transform/);
  await card(page, 'orders').click();
  const edge = page.locator('.edge-path');
  await expect(edge).toHaveAttribute('data-state', 'active');
  await edge.evaluate((el) => el.setAttribute('data-retained', 'yes'));
  const route = () =>
    edge.evaluate((el) => {
      const coords = el
        .getAttribute('d')!
        .match(/-?\d+(?:\.\d+)?/g)!
        .map(Number);
      const labels = [...el.parentElement!.querySelectorAll('g[transform]')].map((g) => {
        const pos = g
          .getAttribute('transform')!
          .match(/-?\d+(?:\.\d+)?/g)!
          .map(Number);
        return { x: pos[0], text: g.querySelector('text')!.textContent };
      });
      return {
        sx: coords[0],
        sy: coords[1],
        cx1: coords[2],
        cx2: coords[4],
        tx: coords[6],
        ty: coords[7],
        labels,
      };
    });
  const before = await route();
  expect(before.labels).toHaveLength(2);
  expect(before.sx).toBe(650); // source's left border
  expect(before.tx).toBe(50 + 284); // target's right border
  expect(before.cx1).toBeLessThan(before.sx);
  expect(before.cx2).toBeGreaterThan(before.tx);
  expect(before.labels.map((l) => l.x)).toEqual([before.sx - 34, before.tx + 34]);
  const zoom = await viewport.evaluate(
    (el) => new DOMMatrix((el as HTMLElement).style.transform).a,
  );
  const clientX = 250 - Math.round(1100 * zoom);
  await card(page, 'orders').dispatchEvent('pointerdown', {
    button: 0,
    pointerId: 20,
    clientX: 250,
    clientY: 300,
  });
  await canvas.dispatchEvent('pointermove', { pointerId: 20, clientX, clientY: 300 });
  // Sides must change during the drag, before the layout is saved.
  await expect.poll(async () => (await route()).tx).toBe(50);
  const during = await route();
  expect(during.sx).toBeLessThan(during.tx);
  expect(during.cx1).toBeGreaterThan(during.sx);
  expect(during.cx2).toBeLessThan(during.tx);
  expect([during.sy, during.ty]).toEqual([before.sy, before.ty]);
  expect(during.labels.map((l) => l.x)).toEqual([during.sx + 34, during.tx - 34]);
  expect(during.labels.map((l) => l.text)).toEqual(before.labels.map((l) => l.text));
  await expect(edge).toHaveAttribute('data-retained', 'yes');
  await canvas.dispatchEvent('pointerup', { pointerId: 20, clientX, clientY: 300 });
  await expect.poll(async () => (await positions(page))[orders.id].x).toBeLessThan(0);
  const saved = await positions(page);
  expect(saved[customers.id]).toEqual({ x: 50, y: 100 });
  await page.reload();
  await expect(edge).toHaveCount(1);
  const restored = await route();
  expect(restored.sx).toBeCloseTo(saved[orders.id].x + 284);
  expect(restored.tx).toBe(50);
});

test('the map is reachable, selectable and movable with the keyboard', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  const id = '["main","orders"]';
  const orders = page.getByRole('button', { name: /^main\.orders, table, 6 columns$/ });
  await expect(orders).toHaveAttribute('aria-pressed', 'false');
  await expect(page.locator('#graph-keys')).toContainText('Enter or Space selects one');

  const before = await positions(page);
  await orders.focus();
  await page.keyboard.press('Enter');
  await expect(orders).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('.entity-node[data-state="dimmed"]')).not.toHaveCount(0);

  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('Shift+ArrowDown');
  await expect
    .poll(async () => {
      const now = await positions(page);
      return [Math.round(now[id].x - before[id].x), Math.round(now[id].y - before[id].y)];
    })
    .toEqual([10, 50]);

  // A selected table is already the inspector's subject, so Enter opens it.
  await page.keyboard.press('Enter');
  await expect(page.locator('.inspector')).toHaveCount(1);
  await page.keyboard.press('Escape');
  await expect(orders).toHaveAttribute('aria-pressed', 'false');
  await expect(page.locator('.entity-node[data-state="dimmed"]')).toHaveCount(0);

  // Selection keeps the card mounted, so focus survives a move past the viewport edge.
  await page.keyboard.press('Enter');
  for (let step = 0; step < 40; step++) await page.keyboard.press('Shift+ArrowDown');
  await expect(orders).toBeFocused();
  await expect
    .poll(async () => Math.round((await positions(page))[id].y - before[id].y))
    .toBe(2050);
  expect(errors).toEqual([]);
});

test('the connect dialog builds a connection string from fields and still accepts a pasted one', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  await page.getByRole('button', { name: 'Connect database' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByLabel(/^Connection name/).fill('Production');
  const connect = dialog.getByRole('button', { name: /Connect & map/ });

  // Nothing to compose yet, so the dialog says what is missing instead of offering to connect.
  await expect(connect).toBeDisabled();
  await dialog.getByLabel(/^Host/).fill('db.internal');
  await dialog.getByLabel(/^Database \*/).fill('shop');
  await dialog.getByLabel(/^User/).fill('alice');
  await dialog.getByLabel(/^Password/).fill('p@ss word#1');

  // The preview shows exactly what will be sent, with the password withheld.
  await expect(
    dialog.getByText('postgresql://alice:••••••••@db.internal/shop?sslmode=require'),
  ).toBeVisible();
  await expect(connect).toBeEnabled();
  await dialog.getByLabel('Encryption').selectOption('verify-full');
  await expect(dialog.getByText(/sslmode=verify-full$/)).toBeVisible();

  // SQL Server uses another format, and refuses a password it cannot quote.
  await dialog.getByRole('radio', { name: 'SQL Server' }).check();
  await expect(
    dialog.getByText(/^Server=tcp:db\.internal;Database=shop;User ID=alice;/),
  ).toBeVisible();
  await dialog.getByLabel(/^Password/).fill('pa}ss');
  await expect(dialog.getByText(/cannot contain/)).toBeVisible();
  await expect(connect).toBeDisabled();

  // A string the user already has is still accepted verbatim.
  await dialog.getByRole('button', { name: 'Connection string', exact: true }).click();
  await expect(dialog.getByLabel(/^Connection string/)).toBeVisible();
  await expect(dialog.locator('#connection-host')).toHaveCount(0);
  await expect(connect).toBeEnabled();

  // SQLite asks for a file, so neither mode applies.
  await dialog.getByRole('radio', { name: 'SQLite' }).check();
  await expect(dialog.getByRole('group', { name: 'How to enter the connection' })).toHaveCount(0);
  await expect(dialog.getByLabel(/^Database file/)).toBeVisible();
  expect(errors).toEqual([]);
});

test('OpenAPI maps arrange routes left of models with orthogonal references', async ({ page }) => {
  const project = structuredClone(sample) as Project;
  const source = project.sources[1];
  source.positions = {};
  project.sources = [source];
  await page.addInitScript(
    ({ key, project }) => {
      if (!localStorage.getItem(key)) localStorage.setItem(key, JSON.stringify([project]));
    },
    { key: storageKey, project },
  );
  await page.goto('/');
  await expect
    .poll(async () => Object.keys(await positions(page)).length)
    .toBe(source.graph.entities.length);
  await expect(cards(page)).toHaveCount(source.graph.entities.length);
  const initial = await positions(page);
  const routes = source.graph.entities.filter((entity) => entity.kind === 'operation');
  const models = source.graph.entities.filter((entity) => entity.kind === 'schema');
  for (const route of routes) {
    for (const model of models) expect(initial[route.id].x + 284).toBeLessThan(initial[model.id].x);
  }
  const positionOf = (name: string) =>
    initial[source.graph.entities.find((entity) => entity.name === name)!.id];
  for (const [route, model, nested] of [
    ['/products', 'Product', undefined],
    ['/customers/{id}', 'Customer', 'Address'],
    ['/orders', 'Order', 'OrderItem'],
  ] as const) {
    expect(positionOf(route).y).toBe(positionOf(model).y);
    if (nested) {
      expect(positionOf(nested).y).toBe(positionOf(model).y);
      expect(positionOf(nested).x).toBeGreaterThan(positionOf(model).x + 284);
    }
  }
  expect(positionOf('Product').x).toBe(positionOf('Order').x);
  expect(positionOf('Error').y).toBeGreaterThan(positionOf('Order').y + 251);
  const paths = await page
    .locator('.edge-path')
    .evaluateAll((edges) => edges.map((edge) => edge.getAttribute('d')!));
  expect(paths).toHaveLength(source.graph.relations.length);
  for (const path of paths) {
    expect(path).toMatch(/^M/);
    expect(path).toContain('H');
    expect(path).toContain('V');
    expect(path).not.toMatch(/[CQ]|NaN|Infinity/);
  }
  await card(page, '/orders').click();
  await expect(page.locator('.edge-path[data-state="active"]')).not.toHaveCount(0);
  await page.getByRole('button', { name: 'Inspect Orders./orders', exact: true }).click();
  await expect(page.locator('.inspector')).toContainText('response 201');
  await page.getByRole('button', { name: 'Close inspector' }).click();
  await page.getByRole('button', { name: 'Auto layout', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Auto layout', exact: true })).toBeEnabled();
  expect(await positions(page)).toEqual(initial);
  await page.reload();
  await expect(cards(page)).toHaveCount(source.graph.entities.length);
  expect(await positions(page)).toEqual(initial);
});
