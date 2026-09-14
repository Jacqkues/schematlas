import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';

const browser = await chromium.launch({ channel: 'chrome', headless: true });
const base = process.env.HERO_TEST_URL ?? 'http://localhost:3000';
try {
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 }, reducedMotion: 'reduce' });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const hydrated = page.waitForRequest("**/api/releases/latest");
  await page.goto(base);
  await hydrated;
  const graph = page.locator('.hero-graph');
  const orders = page.getByRole('button', { name: 'Highlight orders relationships', exact: true });
  assert.equal(await graph.getByRole('button').count(), 5);
  await orders.hover();
  await page.waitForFunction(() => document.querySelectorAll(".hero-graph-edge.is-active").length === 3);
  assert.equal(await page.locator('.hero-graph-edge.is-active').count(), 3);
  assert.equal(await page.locator('.hero-map-node.is-muted').count(), 1);
  assert.equal(await page.locator('.hero-map-node.is-muted').evaluate(el => getComputedStyle(el).opacity), '0.4');
  await orders.click();
  assert.equal(await orders.getAttribute('aria-pressed'), 'true');
  await page.mouse.move(5, 5);
  assert.equal(await page.locator('.hero-graph-scene').getAttribute('data-active'), 'orders');
  await orders.press('Escape');
  assert.equal(await page.locator('.hero-graph-scene').getAttribute('data-active'), 'none');
  const api = page.getByRole('button', { name: 'Highlight GET /customers/{id} relationships', exact: true });
  await api.focus();
  assert.equal(await page.locator('.hero-graph-edge.is-active').count(), 1);
  await api.press('Enter');
  assert.equal(await api.getAttribute('aria-pressed'), 'true');
  await api.press('Escape');
  assert.equal(await orders.evaluate(el => getComputedStyle(el).animationName), 'none');
  await graph.hover({ position: { x: 30, y: 20 } });
  assert.ok(['none', 'matrix(1, 0, 0, 1, 0, 0)'].includes(await page.locator('.hero-graph-scene').evaluate(el => getComputedStyle(el).transform)));
  await page.emulateMedia({ reducedMotion: 'no-preference' });
  await graph.hover({ position: { x: 100, y: 100 } });
  await page.waitForFunction(() => document.querySelector('.hero-graph-scene').style.transform !== 'translate(0px, 0px)');
  await page.mouse.move(5, 5);
  await page.waitForFunction(() => document.querySelector('.hero-graph-scene').style.transform === 'translate(0px, 0px)');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  for (const width of [320, 390, 768, 1024, 1440]) {
    await page.setViewportSize({ width, height: 900 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, `No horizontal overflow at ${width}px`);
    const bounds = await graph.boundingBox();
    for (const button of await graph.getByRole('button').all()) {
      const node = await button.boundingBox();
      assert.ok(node.x >= bounds.x - 1 && node.x + node.width <= bounds.x + bounds.width + 1, 'Nodes fit within the graph');
    }
  }
  const touch = await browser.newContext({ viewport: { width: 390, height: 844 }, hasTouch: true, isMobile: true, reducedMotion: 'reduce' });
  const mobile = await touch.newPage();
  const mobileHydrated = mobile.waitForRequest("**/api/releases/latest");
  await mobile.goto(base);
  await mobileHydrated;
  const payments = mobile.getByRole('button', { name: 'Highlight payments relationships', exact: true });
  await payments.tap();
  assert.equal(await payments.getAttribute('aria-pressed'), 'true');
  assert.equal(await mobile.locator('.hero-graph-edge.is-active').count(), 1);
  assert.equal(await mobile.locator('.hero-graph').evaluate(el => getComputedStyle(el).touchAction), 'auto');
  await touch.close();
  assert.deepEqual(errors, []);
  console.log('PASS: hover/keyboard/touch relationships, selection/reset, pointer response, reduced motion and responsive graph bounds.');
} finally { await browser.close(); }
