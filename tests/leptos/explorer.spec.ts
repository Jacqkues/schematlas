import { test, expect, type Page } from '@playwright/test';
import { newProject, openProject } from './support';

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
  await page.getByRole('combobox', { name: 'Location', exact: true }).click();
  await page.getByRole('option', { name: destination, exact: true }).click();
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
  await expect(page.getByRole('combobox', { name: 'Location', exact: true })).toHaveText(
    'Checkout',
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
  await openProject(page);
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
  await page.getByRole('combobox', { name: 'Location', exact: true }).click();
  await page.getByRole('option', { name: 'Documents', exact: true }).click();
  await page.getByRole('button', { name: 'Save folder' }).click();
  await expect(
    branch(page, 'Documents').getByRole('button', { name: 'Features', exact: true }),
  ).toBeVisible();
  await files(page).getByRole('button', { name: 'Manage Documents folder' }).click();
  await expect(page.getByRole('button', { name: 'Delete folder', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await files(page).getByRole('button', { name: 'Manage Features folder' }).click();
  await page.getByRole('combobox', { name: 'Location', exact: true }).click();
  await expect(page.getByRole('option', { name: 'Documents', exact: true })).toBeVisible();
  await expect(page.getByRole('option', { name: 'Documents / Features', exact: true })).toHaveCount(
    0,
  );
  await page.getByRole('combobox', { name: 'Location', exact: true }).press('Escape');
  await page.getByRole('button', { name: 'Delete folder', exact: true }).click();
  await expect(folder(page, 'Features')).toHaveCount(0);
});

test('new documents use the chosen folder and folders stay scoped to their project', async ({
  page,
}) => {
  await demo(page);
  await createFolder(page, 'Checkout');
  await page.getByRole('button', { name: 'New doc', exact: true }).click();
  await page.getByLabel('Title').fill('Checkout notes');
  await page.getByRole('combobox', { name: 'Location', exact: true }).click();
  await page.getByRole('option', { name: 'Checkout', exact: true }).click();
  await page.getByRole('button', { name: 'Create', exact: true }).click();
  await expect(
    branch(page, 'Checkout').getByRole('button', { name: 'Checkout notes', exact: true }),
  ).toBeVisible();
  await expect(page.locator('article.markdown h1')).toHaveText('Checkout notes');
  await move(page, 'Checkout notes', 'Project root');
  await expect(
    files(page).locator(':scope > ul > li > div > button').filter({ hasText: 'Checkout notes' }),
  ).toHaveCount(1);
  const projects = await page.evaluate((key) => JSON.parse(localStorage.getItem(key)!), projectKey);
  expect(projects[0].explorer.folders.some((f: any) => f.name === 'Checkout')).toBe(false);
  const atlas = await page.evaluate(
    () =>
      Object.values(
        JSON.parse(localStorage.getItem('schema-atlas-browser-preview-atlas-v2')!),
      )[0] as any,
  );
  expect(atlas.toc.items.find((e: any) => e.title === 'Checkout notes').file).toBe(
    '.schematlas/checkout/checkout-notes.md',
  );
  expect(atlas.docs['.schematlas/checkout/checkout-notes.md']).toContain('# Checkout notes');
  await files(page).getByRole('button', { name: 'Move Checkout notes', exact: true }).click();
  await page.getByRole('button', { name: 'Remove from TOC', exact: true }).click();
  await expect(
    files(page).getByRole('button', { name: 'Add Checkout notes to TOC' }),
  ).toBeVisible();
  await files(page).getByRole('button', { name: 'Add Checkout notes to TOC' }).click();
  await page.getByRole('button', { name: 'Move', exact: true }).click();
  await expect(files(page).getByRole('button', { name: 'Add Checkout notes to TOC' })).toHaveCount(
    0,
  );
  await newProject(page, 'Another project');
  await expect(folder(page, 'Checkout')).toHaveCount(0);
  await page.getByRole('combobox', { name: 'Current project' }).click();
  await page.getByRole('option', { name: 'Northstar Commerce', exact: true }).click();
  await expect(folder(page, 'Checkout')).toBeVisible();
  await expect(
    files(page).getByRole('button', { name: 'Checkout notes', exact: true }),
  ).toBeVisible();
});

test('toc controls mixed entry order and custom titles after external edits', async ({ page }) => {
  await demo(page);
  await page.getByRole('button', { name: 'Create toc.yaml', exact: true }).click();
  await page.evaluate(() => {
    const key = 'schema-atlas-browser-preview-atlas-v2';
    const all = JSON.parse(localStorage.getItem(key)!);
    const atlas = Object.values(all)[0] as any;
    atlas.toc = {
      version: 1,
      items: [
        { title: 'Start here', file: '.schematlas/docs/overview.md' },
        {
          title: 'Checkout',
          children: [
            { title: 'Storage', source: 'Commerce database', type: 'database' },
            { title: 'System map', file: 'docs/architecture.mmd', type: 'diagram' },
          ],
        },
        { title: 'Missing note', file: 'not-yet-written.md' },
      ],
    };
    localStorage.setItem(key, JSON.stringify(all));
  });
  await page.reload();
  await openProject(page);
  const root = files(page).locator(':scope > ul > li > div > button:first-child');
  await expect(root).toHaveText(['Start here', 'Checkout', 'Missing note']);
  const children = branch(page, 'Checkout').locator(':scope > ul > li > div > button:first-child');
  await expect(children).toHaveText(['Storage', 'System map']);
  await files(page).getByRole('button', { name: 'Start here', exact: true }).click();
  await expect(page.locator('article.markdown h1')).toHaveText('Northstar Commerce');
  await files(page).getByRole('button', { name: 'Missing note', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Missing documentation file');
  await files(page).getByRole('button', { name: 'System map', exact: true }).click();
  await expect(
    page.getByLabel('Interactive diagram', { exact: true }).locator('.diagram-node'),
  ).toHaveCount(5);
});

test('a stale folder edit cannot overwrite externally changed toc content', async ({ page }) => {
  await demo(page);
  await page.getByRole('button', { name: 'Create toc.yaml', exact: true }).click();
  await page.getByRole('button', { name: 'New folder', exact: true }).click();
  await page.getByLabel('Folder name').fill('Stale edit');
  await page.evaluate(() => {
    const key = 'schema-atlas-browser-preview-atlas-v2';
    const all = JSON.parse(localStorage.getItem(key)!);
    (Object.values(all)[0] as any).toc.items.push({ title: 'Added by editor', children: [] });
    localStorage.setItem(key, JSON.stringify(all));
  });
  await page.getByRole('button', { name: 'Create folder', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('toc.yaml changed since it was read');
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.reload();
  await openProject(page);
  await expect(folder(page, 'Added by editor')).toBeVisible();
  await expect(folder(page, 'Stale edit')).toHaveCount(0);
});
