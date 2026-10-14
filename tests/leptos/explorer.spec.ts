import { test, expect, type Page } from '@playwright/test';

const projectKey = 'schema-atlas-browser-preview-v1';
const files = (page: Page) => page.getByRole('navigation', { name: 'Project files' });
const folder = (page: Page, name: string) => files(page).getByRole('button', { name, exact: true });
const branch = (page: Page, name: string) => folder(page, name).locator('xpath=../..');

async function demo(page: Page) {
  await page.goto('/');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await expect(folder(page, 'Diagrams')).toBeVisible();
}

async function createFolder(page: Page, name: string) {
  await page.getByRole('button', { name: 'New folder', exact: true }).click();
  await page.getByLabel('Folder name').fill(name);
  await page.getByRole('button', { name: 'Create folder', exact: true }).click();
  await expect(folder(page, name)).toBeVisible();
}

async function move(page: Page, title: string, destination: string) {
  await files(page)
    .getByRole('button', { name: `Move ${title}`, exact: true })
    .click();
  await page.getByLabel('Location', { exact: true }).selectOption({ label: destination });
  await page.getByRole('button', { name: 'Move', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
}

test('folders nest, organize mixed items and persist without changing their content', async ({
  page,
}) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await demo(page);
  await createFolder(page, 'Checkout');
  await files(page).getByRole('button', { name: 'Manage Checkout folder' }).click();
  await page.getByRole('button', { name: 'New subfolder' }).click();
  await page.getByLabel('Folder name').fill('Reference');
  await expect(page.getByLabel('Location', { exact: true })).toHaveValue(
    await folder(page, 'Checkout').evaluate(() => {
      const project = JSON.parse(localStorage.getItem('schema-atlas-browser-preview-v1')!)[0];
      return project.explorer.folders.find((f: any) => f.name === 'Checkout').id;
    }),
  );
  await page.getByRole('button', { name: 'Create folder', exact: true }).click();
  await expect(
    branch(page, 'Checkout').getByRole('button', { name: 'Reference', exact: true }),
  ).toBeVisible();
  await move(page, 'Northstar architecture', 'Checkout');
  await move(page, 'Commerce database', 'Checkout / Reference');
  await expect(
    branch(page, 'Checkout').getByRole('button', { name: 'Northstar architecture', exact: true }),
  ).toBeVisible();
  await expect(
    branch(page, 'Reference').getByRole('button', { name: 'Commerce database', exact: true }),
  ).toBeVisible();
  await folder(page, 'Checkout').click();
  await expect(folder(page, 'Checkout')).toHaveAttribute('aria-expanded', 'false');
  await expect(
    files(page).getByRole('button', { name: 'Northstar architecture', exact: true }),
  ).not.toBeVisible();
  await folder(page, 'Checkout').press('ArrowRight');
  await files(page).getByRole('button', { name: 'Northstar architecture', exact: true }).click();
  await expect(
    page.getByLabel('Interactive diagram', { exact: true }).locator('.diagram-node'),
  ).toHaveCount(5);
  await page.reload();
  await expect(
    branch(page, 'Reference').getByRole('button', { name: 'Commerce database', exact: true }),
  ).toBeVisible();
  await files(page).getByRole('button', { name: 'Commerce database', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Commerce database', exact: true })).toBeVisible();
  expect(errors).toEqual([]);
});

test('folder management rejects duplicates and protects nonempty folders', async ({ page }) => {
  await demo(page);
  await createFolder(page, 'Checkout');
  await page.getByRole('button', { name: 'New folder', exact: true }).click();
  await page.getByLabel('Folder name').fill('checkout');
  await page.getByRole('button', { name: 'Create folder', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveText('A folder with this name already exists here.');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await files(page).getByRole('button', { name: 'Manage Checkout folder' }).click();
  await page.getByLabel('Folder name').fill('Features');
  await page.getByLabel('Location', { exact: true }).selectOption({ label: 'Documents' });
  await page.getByRole('button', { name: 'Save folder' }).click();
  await expect(
    branch(page, 'Documents').getByRole('button', { name: 'Features', exact: true }),
  ).toBeVisible();
  await files(page).getByRole('button', { name: 'Manage Documents folder' }).click();
  await expect(page.getByRole('button', { name: 'Delete folder', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await files(page).getByRole('button', { name: 'Manage Features folder' }).click();
  await expect(
    page
      .getByLabel('Location', { exact: true })
      .locator('option')
      .filter({ hasText: 'Documents / Features' }),
  ).toHaveCount(0);
  await page.getByRole('button', { name: 'Delete folder', exact: true }).click();
  await expect(folder(page, 'Features')).toHaveCount(0);
});

test('new documents use the chosen folder and folders stay scoped to their project', async ({
  page,
}) => {
  await demo(page);
  await createFolder(page, 'Checkout');
  await files(page).getByRole('button', { name: 'New doc', exact: true }).click();
  await page.getByLabel('Title').fill('Checkout notes');
  await page.getByLabel('Location', { exact: true }).selectOption({ label: 'Checkout' });
  await page.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(
    branch(page, 'Checkout').getByRole('button', { name: 'Checkout notes', exact: true }),
  ).toBeVisible();
  await expect(page.locator('article.markdown h1')).toHaveText('Checkout notes');
  await move(page, 'Checkout notes', 'Project root');
  await expect(
    files(page).locator(':scope > ul > li > button').filter({ hasText: 'Checkout notes' }),
  ).toHaveCount(1);
  const projects = await page.evaluate((key) => JSON.parse(localStorage.getItem(key)!), projectKey);
  expect(projects[0].explorer.locations['doc:checkout-notes']).toBeNull();
  await page.getByRole('button', { name: 'New project', exact: true }).click();
  await page.getByLabel('Project name').fill('Another project');
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(folder(page, 'Checkout')).toHaveCount(0);
  await page.getByLabel('Current project').selectOption({ label: 'Northstar Commerce' });
  await expect(folder(page, 'Checkout')).toBeVisible();
  await expect(
    files(page).getByRole('button', { name: 'Checkout notes', exact: true }),
  ).toBeVisible();
});
