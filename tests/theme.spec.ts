import { test, expect } from '@playwright/test';

test('light theme persists and preserves the active graph and opaque table cards', async ({
  page,
}, testInfo) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/');
  await page.getByRole('button', { name: 'Light theme', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await expect(page.locator('.entity-node')).toHaveCount(6);
  await expect(page.locator('.entity-node').first()).toHaveCSS(
    'background-color',
    'rgb(255, 255, 255)',
  );
  const positions = await page
    .locator('.svelte-flow__node-entity')
    .evaluateAll((nodes) => nodes.map((n) => (n as HTMLElement).style.transform));
  await page.getByRole('button', { name: 'Dark theme', exact: true }).click();
  await expect(page.locator('.svelte-flow')).toHaveClass(/dark/);
  await page.getByRole('button', { name: 'Light theme', exact: true }).click();
  expect(
    await page
      .locator('.svelte-flow__node-entity')
      .evaluateAll((nodes) => nodes.map((n) => (n as HTMLElement).style.transform)),
  ).toEqual(positions);
  await page.getByRole('button', { name: 'Toggle minimap' }).click();
  await expect(page.locator('.svelte-flow__minimap')).toBeVisible();
  await page.screenshot({ animations: 'disabled', path: testInfo.outputPath('light-graph.png') });
  await page.reload();
  await expect(page.getByRole('button', { name: 'Light theme', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  await page.getByRole('button', { name: 'New project', exact: false }).click();
  await expect(page.getByRole('dialog')).toHaveCSS('background-color', 'rgb(255, 255, 255)');
  await page.screenshot({ animations: 'disabled', path: testInfo.outputPath('light-dialog.png') });
  expect(errors).toEqual([]);
});

test('theme switch still works when preference storage is unavailable', async ({
  page,
}, testInfo) => {
  await page.addInitScript(() => {
    const original = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key, value) {
      if (key === 'schematlas.theme') throw new DOMException('Storage unavailable');
      original.call(this, key, value);
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Light theme', exact: true }).click();
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'light');
  await page.getByRole('button', { name: 'Dark theme', exact: true }).click();
  await expect(page.locator('html')).toHaveCSS('color-scheme', 'dark');
  await page.screenshot({ animations: 'disabled', path: testInfo.outputPath('dark-home.png') });
});
