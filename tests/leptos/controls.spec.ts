import { test, expect } from '@playwright/test';
import { openProject } from './support';

test('inputs show focus and legible text selection in both themes', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  for (const theme of ['dark', 'light']) {
    if (theme === 'light')
      await page.getByRole('button', { name: 'Switch to light theme' }).click();
    await page.getByRole('button', { name: 'All projects' }).first().click();
    await page.getByRole('button', { name: 'Empty project', exact: true }).click();
    const input = page.getByLabel('Project name');
    const idleBorder = await input.evaluate((el) => getComputedStyle(el).borderColor);
    await input.fill('A clearly selected field');
    await input.focus();
    await expect(input).toBeFocused();
    expect(await input.evaluate((el) => getComputedStyle(el).borderColor)).not.toBe(idleBorder);
    expect(await input.evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe('none');
    const selection = await input.evaluate((el) => ({
      background: getComputedStyle(el, '::selection').backgroundColor,
      color: getComputedStyle(el, '::selection').color,
      userSelect: getComputedStyle(el).userSelect,
    }));
    expect(selection.background).not.toBe('rgba(0, 0, 0, 0)');
    expect(selection.background).not.toBe(selection.color);
    expect(selection.userSelect).toBe('text');
    await input.press(process.platform === 'darwin' ? 'Meta+A' : 'Control+A');
    await page.screenshot({ path: test.info().outputPath(`input-${theme}.png`) });
    await page.getByRole('button', { name: 'Close dialog' }).click();
    await openProject(page);
  }
  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  const composer = page.getByLabel('Message your agent');
  const card = composer.locator('..');
  const idle = await card.evaluate((el) => getComputedStyle(el).borderColor);
  await composer.fill('A focused prompt');
  expect(await card.evaluate((el) => getComputedStyle(el).borderColor)).not.toBe(idle);
  expect(await card.evaluate((el) => getComputedStyle(el).boxShadow)).not.toBe('none');
});

test('styled choices support keyboard navigation, cancellation and pointer selection inside dialogs', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Take a look around with an example project' }).click();
  await page.getByRole('button', { name: 'New doc', exact: true }).click();
  const dialog = page.getByRole('dialog');
  const location = dialog.getByRole('combobox', { name: 'Location' });
  await location.focus();
  await location.press('ArrowDown');
  await expect(location).toHaveAttribute('aria-expanded', 'true');
  await expect(dialog.getByRole('option', { name: 'Project root', exact: true })).toHaveAttribute(
    'aria-selected',
    'true',
  );
  await location.press('End');
  const active = await location.getAttribute('aria-activedescendant');
  await expect(dialog.locator(`[id="${active}"]`)).toHaveAttribute('data-active', '');
  await location.press('Escape');
  await expect(location).toHaveAttribute('aria-expanded', 'false');
  await expect(location).toContainText('Project root');
  await expect(dialog).toBeVisible();
  await location.press('d');
  await location.press('Enter');
  await expect(location).toContainText('Database schemas');
  await expect(location).toBeFocused();
  await location.click();
  const menu = dialog.getByRole('listbox', { name: 'Location options' });
  await expect(menu).toBeVisible();
  const box = (await menu.boundingBox())!;
  const anchor = (await location.boundingBox())!;
  expect(Math.abs(box.x - anchor.x)).toBeLessThan(3);
  expect(
    Math.min(Math.abs(box.y - anchor.y - anchor.height), Math.abs(box.y + box.height - anchor.y)),
  ).toBeLessThan(10);
  await page.screenshot({ path: test.info().outputPath('location-options.png') });
  await dialog.getByRole('option', { name: 'Documents', exact: true }).click();
  await expect(location).toContainText('Documents');
  await location.click();
  await dialog.getByLabel(/^Title/).click();
  await expect(location).toHaveAttribute('aria-expanded', 'false');
  await expect(dialog).toBeVisible();
  await page.setViewportSize({ width: 900, height: 600 });
  await location.click();
  await expect(menu).toBeVisible();
  const smallBox = (await menu.boundingBox())!;
  expect(smallBox.y).toBeGreaterThanOrEqual(0);
  expect(smallBox.y + smallBox.height).toBeLessThanOrEqual(600);
  await page.screenshot({ path: test.info().outputPath('location-small-window.png') });
});
