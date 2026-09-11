import { test, expect } from '@playwright/test';
test('project lifecycle, demo graphs, inspection, search, saved layout, and export', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await expect(
    page.getByRole('heading', { name: 'Complex systems. Clear connections.' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Create your first project' }).click();
  await page.getByLabel('Project name').fill('Integration workspace');
  await page.getByLabel('Description').fill('A test project');
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(
    page.getByRole('heading', { name: 'Integration workspace starts here.' }),
  ).toBeVisible();
  await page.reload();
  await expect(
    page.getByRole('heading', { name: 'Integration workspace starts here.' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await expect(page.locator('.entity-node')).toHaveCount(6);
  await page
    .locator('.entity-title strong')
    .filter({ hasText: /^orders$/ })
    .click();
  await expect(page.locator('.inspector')).toHaveCount(0);
  await page.getByRole('button', { name: 'Inspect main.orders', exact: true }).click();
  await expect(page.locator('.inspector h2')).toHaveText('orders');
  await expect(page.locator('.inspector')).toContainText('customer_id');
  await page.getByRole('button', { name: 'Close inspector' }).click();
  await page.getByRole('searchbox', { name: 'Search schema' }).fill('not-a-table');
  await expect(page.getByRole('heading', { name: 'No matching nodes' })).toBeVisible();
  await page.getByRole('searchbox', { name: 'Search schema' }).fill('');
  await expect(page.locator('.entity-node')).toHaveCount(6);
  await page.getByRole('button', { name: 'Auto layout', exact: true }).click();
  await page.getByRole('button', { name: 'Toggle minimap' }).click();
  await expect(page.locator('.svelte-flow__minimap')).toHaveCount(0);
  await page.getByRole('button', { name: 'Commerce API 9' }).click();
  await expect(page.locator('.entity-node')).toHaveCount(9);
  await page.getByRole('button', { name: 'Commerce database 6' }).click();
  await page.screenshot({ path: 'tests/database-map.png' });
  const download = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Export', exact: true }).click();
  expect((await download).suggestedFilename()).toBe('schema-map.json');
  await page.getByRole('button', { name: 'Delete source', exact: true }).click();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(page.locator('.entity-node')).toHaveCount(6);
  expect(errors).toEqual([]);
});
test('dialogs have native keyboard dismissal and validate project name', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'New project', exact: false }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByRole('dialog')).toHaveCount(0);
});
