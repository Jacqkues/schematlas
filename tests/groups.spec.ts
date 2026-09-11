import { test, expect, type Page } from '@playwright/test';
import type { Source } from '../src/lib/types';

async function savedSource(page: Page): Promise<Source> {
  return page.evaluate(
    () => JSON.parse(localStorage.getItem('schema-atlas-browser-preview-v1')!)[0].sources[0],
  );
}

test('colored groups move their members together, persist, and support undo', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await expect(page.locator('.entity-node')).toHaveCount(6);
  await page.getByRole('button', { name: 'Groups', exact: true }).click();
  await page.getByRole('button', { name: 'New group', exact: true }).click();
  await page.getByLabel('Group name', { exact: true }).fill('Billing');
  await page.getByRole('button', { name: 'Blue group color', exact: true }).click();
  await page.getByRole('checkbox', { name: 'orders main', exact: true }).check();
  await page.getByRole('checkbox', { name: 'payments main', exact: true }).check();
  await page.getByRole('button', { name: 'Create group', exact: true }).click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  let saved = await savedSource(page);
  const group = saved.groups!.find((g) => g.name === 'Billing')!;
  expect(group.color).toBe('#6d9de3');
  await page.getByRole('button', { name: 'Close map options', exact: true }).click();
  const move = page.getByRole('button', { name: 'Move group Billing', exact: true });
  await expect(move).toBeVisible();
  await page.getByRole('button', { name: 'Fit graph to screen', exact: true }).click();
  await page.waitForTimeout(350);
  const box = await move.boundingBox();
  const before = structuredClone(saved.positions);
  await page.mouse.move(box!.x + 90, box!.y + box!.height / 2);
  await page.mouse.down();
  await page.mouse.move(box!.x + 200, box!.y + 70, { steps: 12 });
  await page.mouse.up();
  await expect
    .poll(async () => (await savedSource(page)).positions[group.nodeIds[0]].x)
    .not.toBe(before[group.nodeIds[0]].x);
  saved = await savedSource(page);
  const delta = {
    x: saved.positions[group.nodeIds[0]].x - before[group.nodeIds[0]].x,
    y: saved.positions[group.nodeIds[0]].y - before[group.nodeIds[0]].y,
  };
  expect(delta.x).toBeGreaterThan(10);
  for (const id of group.nodeIds) {
    expect(saved.positions[id].x - before[id].x).toBeCloseTo(delta.x);
    expect(saved.positions[id].y - before[id].y).toBeCloseTo(delta.y);
  }
  for (const entity of saved.graph.entities.filter((e) => !group.nodeIds.includes(e.id))) {
    expect(saved.positions[entity.id]).toEqual(before[entity.id]);
  }
  await page.reload();
  await page.getByRole('button', { name: 'Groups', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit group Billing', exact: true })).toBeVisible();
  expect((await savedSource(page)).positions).toEqual(saved.positions);
  await page.getByRole('button', { name: 'Undo canvas edit', exact: true }).click();
  await expect.poll(async () => (await savedSource(page)).positions).toEqual(before);
  // Existing groups can change color without losing membership.
  await page.getByRole('button', { name: 'Edit group Billing', exact: true }).click();
  await page.getByRole('button', { name: 'Rose group color', exact: true }).click();
  await page.getByRole('button', { name: 'Save group', exact: true }).click();
  await expect
    .poll(async () => (await savedSource(page)).groups!.find((g) => g.id === group.id)!.color)
    .toBe('#d48b9e');
  // A keyboard move is also a complete, persisted group move.
  await page.getByRole('button', { name: 'Close map options', exact: true }).click();
  await move.focus();
  await page.keyboard.press('ArrowRight');
  await expect
    .poll(async () => (await savedSource(page)).positions[group.nodeIds[0]].x)
    .toBe(before[group.nodeIds[0]].x + 10);
  expect(errors).toEqual([]);
});
