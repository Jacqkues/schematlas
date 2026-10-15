import { expect, type Page } from '@playwright/test';

/** The app opens on the projects page; open a project from it (the first, or the named one). */
export async function openProject(page: Page, name?: string) {
  const list = page.getByRole('list', { name: 'Projects' });
  await expect(list).toBeVisible();
  const cards = list.getByRole('button');
  await (name ? cards.filter({ hasText: name }).first() : cards.first()).click();
  await expect(list).toHaveCount(0);
}

/** Start an empty project from the projects page. */
export async function newProject(page: Page, name: string) {
  await page.getByRole('button', { name: 'All projects' }).first().click();
  await page.getByRole('button', { name: 'Empty project', exact: true }).click();
  await page.getByLabel('Project name').fill(name);
  await page.getByRole('button', { name: 'Create project', exact: true }).click();
}
