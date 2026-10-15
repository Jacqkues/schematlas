import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
const base = process.env.DEMO_TEST_URL ?? 'http://localhost:3000';
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const errors = [];
try {
  const page = await browser.newPage({
    viewport: { width: 1440, height: 1050 },
    reducedMotion: 'no-preference',
  });
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(base);
  await page.locator('#workspace').scrollIntoViewIfNeeded();
  await page.locator('.m-aurora[data-renderer="webgl"][data-motion="playing"]').first().waitFor();
  const shaderTime = () =>
    page
      .locator('.m-aurora canvas')
      .first()
      .evaluate((canvas) => {
        const gl = canvas.getContext('webgl');
        return gl.getUniform(
          gl.getParameter(gl.CURRENT_PROGRAM),
          gl.getUniformLocation(gl.getParameter(gl.CURRENT_PROGRAM), 'u_time'),
        );
      });
  const first = await shaderTime();
  await page.waitForTimeout(220);
  assert.ok((await shaderTime()) > first, 'The actual GPU uniform advances');
  await page.screenshot({ path: '/private/tmp/schematlas-aurora-desktop.png', fullPage: false });
  await page
    .getByRole('button', { name: 'Pause background animation', exact: true })
    .first()
    .click();
  await page.locator('.m-aurora[data-motion="paused"]').first().waitFor();
  await page.waitForTimeout(100);
  const paused = await shaderTime();
  await page.waitForTimeout(200);
  assert.equal(await shaderTime(), paused, 'Pausing freezes the actual rendered shader');
  await page
    .getByRole('button', { name: 'Play background animation', exact: true })
    .first()
    .click();
  await page.locator('.m-aurora[data-motion="playing"]').first().waitFor();
  await page.locator('#storage').scrollIntoViewIfNeeded();
  await page.waitForFunction(
    () => document.querySelector('.m-aurora').getBoundingClientRect().bottom < 0,
  );
  await page.waitForTimeout(150);
  const offscreen = await shaderTime();
  await page.waitForTimeout(180);
  assert.equal(await shaderTime(), offscreen, 'Offscreen artwork does not keep rendering');
  await page.locator('#workspace').scrollIntoViewIfNeeded();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page
    .getByRole('button', { name: 'Animation reduced by your preference' })
    .first()
    .waitFor();
  assert.equal(
    await page
      .getByRole('button', { name: 'Animation reduced by your preference' })
      .first()
      .isDisabled(),
    true,
  );
  await page.locator('.m-aurora[data-motion="paused"]').first().waitFor();
  await page.setViewportSize({ width: 390, height: 1000 });
  await page.screenshot({ path: '/private/tmp/schematlas-aurora-mobile.png', fullPage: false });
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.close();
  const fallback = await browser.newPage({
    viewport: { width: 1440, height: 1000 },
    reducedMotion: 'reduce',
  });
  fallback.on('pageerror', (error) => errors.push(error.message));
  await fallback.addInitScript(() => {
    const original = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
      if (kind === 'webgl' || kind === 'webgl2') return null;
      return original.call(this, kind, ...args);
    };
  });
  await fallback.goto(base);
  await fallback
    .locator('.m-aurora[data-renderer="fallback"]')
    .first()
    .waitFor({ state: 'attached' });
  // Without WebGL the canvas stays transparent over the CSS gradient.
  assert.equal(
    await fallback
      .locator('.m-aurora canvas')
      .first()
      .evaluate((el) => getComputedStyle(el).opacity),
    '0',
  );
  await fallback.screenshot({
    path: '/private/tmp/schematlas-aurora-fallback.png',
    fullPage: false,
  });
  await fallback.close();
  assert.deepEqual(errors, []);
  console.log(
    'PASS: actual WebGL animation, pause/resume, offscreen suspension, reduced motion, mobile layout and no-WebGL fallback.',
  );
} finally {
  await browser.close();
}
