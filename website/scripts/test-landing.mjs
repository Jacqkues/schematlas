import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';

const base = process.env.DEMO_TEST_URL ?? 'http://localhost:3000';
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const failures = [];
try {
  for (const width of [1440, 768, 390, 320]) {
    const page = await browser.newPage({
      viewport: { width, height: 1000 },
      reducedMotion: 'reduce',
    });
    page.on('pageerror', (error) => failures.push(error.message));
    await page.goto(base);
    await page.locator('.m-aurora[data-motion]').first().waitFor({ state: 'attached' });
    await page
      .getByRole('heading', { level: 1, name: 'Your codebase. A clearer picture.' })
      .waitFor();
    assert.equal(await page.locator('h1').evaluate((el) => getComputedStyle(el).opacity), '1');
    assert.equal(
      await page
        .locator('h1 span')
        .first()
        .evaluate((el) => getComputedStyle(el).opacity),
      '1',
    );
    const appearance = await page.evaluate(() => ({
      scheme: getComputedStyle(document.documentElement).colorScheme,
      background: getComputedStyle(document.body).backgroundColor,
      headline: getComputedStyle(document.querySelector('h1')).fontFamily,
    }));
    assert.equal(appearance.scheme, 'light');
    assert.equal(appearance.background, 'rgb(251, 250, 248)');
    assert.ok(appearance.headline.includes('Manrope'));
    await page.screenshot({
      path: `/private/tmp/schematlas-landing-hero-${width}.png`,
      fullPage: false,
    });
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - innerWidth);
    assert.ok(overflow <= 1, `No horizontal page overflow at ${width}px (got ${overflow}px)`);
    assert.equal(
      await page.locator('.demo-iframe').count(),
      0,
      'The large WASM playground stays deferred',
    );
    await page.getByRole('link', { name: 'Take the tour', exact: true }).click();
    // One recording follows the scroll; the steps are its chapters and scroll to them.
    const tabs = page.getByRole('list', { name: 'Tour steps' });
    assert.equal(await tabs.getByRole('button').count(), 5);
    await page.waitForFunction(() =>
      document.querySelector('.m-tour video')?.getAttribute('src')?.startsWith('blob:'),
    );
    await page.waitForFunction(() => {
      const video = document.querySelector('.m-tour video');
      return video.readyState >= 2 && video.seekable.length && video.seekable.end(0) > 40;
    });
    const beforeFrame = await page.locator('.m-tour video').evaluate((video) => {
      const canvas = document.createElement('canvas');
      canvas.width = 160;
      canvas.height = 100;
      canvas.getContext('2d').drawImage(video, 0, 0, 160, 100);
      return canvas.toDataURL();
    });
    await tabs.getByRole('button', { name: /Know when the code moves/ }).click();
    await tabs
      .locator('button[aria-pressed="true"]', { hasText: 'Know when the code moves' })
      .waitFor();
    await page.waitForFunction(() => {
      const time = document.querySelector('.m-tour video').currentTime;
      return time > 28 && time < 40;
    });
    await page.waitForFunction((before) => {
      const video = document.querySelector('.m-tour video');
      if (video.seeking || video.readyState < 2) return false;
      const canvas = document.createElement('canvas');
      canvas.width = 160;
      canvas.height = 100;
      canvas.getContext('2d').drawImage(video, 0, 0, 160, 100);
      return canvas.toDataURL() !== before;
    }, beforeFrame);
    // Real scrolling must also seek backwards and display an earlier decoded frame.
    await page.evaluate(() => scrollBy(0, -800));
    await page.waitForFunction(() => {
      const video = document.querySelector('.m-tour video');
      return !video.seeking && video.currentTime > 15 && video.currentTime < 25;
    });
    await page.screenshot({
      path: `/private/tmp/schematlas-landing-tour-${width}.png`,
      fullPage: false,
    });
    await page.getByRole('navigation', { name: 'Main navigation' }).getByRole('link', { name: 'Download', exact: true }).click();
    const other = page.getByRole('button', { name: 'Other platforms', exact: true });
    if (await other.count()) await other.click();
    await page.getByRole('heading', { name: 'macOS', exact: true }).waitFor();
    assert.equal(await page.locator('.m-platforms .download-link').count(), 6);
    await page.close();
    console.log(`Tour, downloads, and layout passed at ${width}px`);
  }
  assert.deepEqual(failures, []);
} finally {
  await browser.close();
}
