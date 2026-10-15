import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
const base = process.env.DEMO_TEST_URL ?? 'http://localhost:5173';
const browser = await chromium.launch({ channel: 'chrome', headless: true });
try {
  const page = await browser.newPage({
    viewport: { width: 390, height: 844 },
    reducedMotion: 'reduce',
  });
  const errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(() =>
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        writeText: async (value) => {
          window.copiedInstallPrompt = value;
        },
      },
    }),
  );
  await page.goto(base);
  await page.getByRole('button', { name: 'Other platforms', exact: true }).waitFor();
  await page.getByRole('link', { name: 'Install with your agent' }).click();
  await page.getByRole('button', { name: 'Ask Codex to install', exact: true }).click();
  await page.getByRole('button', { name: 'Prompt copied', exact: true }).waitFor();
  assert.match(
    await page.evaluate(() => window.copiedInstallPrompt),
    new RegExp(new URL('/install.md', base).href.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')),
  );
  assert.match(await page.evaluate(() => window.copiedInstallPrompt), /from source/);
  await page.getByRole('button', { name: 'Claude Code', exact: true }).click();
  await page.getByRole('button', { name: 'Ask Claude Code to install', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'Paste it into Claude Code' }).waitFor();
  const guide = await page.request.get(new URL('/install.md', base).href);
  assert.equal(guide.status(), 200);
  const markdown = await guide.text();
  for (const required of [
    '# Install Schematlas',
    'macOS',
    'Windows',
    'Linux',
    'wasm32-unknown-unknown',
    '0.21.14',
    'npm ci',
    'npm run desktop:build',
    'AGENTS.md',
  ])
    assert.ok(markdown.includes(required), required);
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.screenshot({ path: '/private/tmp/schematlas-agent-install-mobile.png' });
  await page.close();
  const fallback = await browser.newPage({ reducedMotion: 'reduce' });
  await fallback.addInitScript(() =>
    Object.defineProperty(navigator, 'clipboard', {
      value: {
        writeText: async () => {
          throw new Error('Clipboard denied');
        },
      },
    }),
  );
  await fallback.goto(base);
  await fallback.getByRole('button', { name: 'Other platforms', exact: true }).waitFor();
  await fallback.getByRole('button', { name: 'Ask Codex to install', exact: true }).click();
  const field = fallback.getByRole('textbox', { name: 'Installation prompt' });
  await field.waitFor();
  assert.ok((await field.inputValue()).includes(new URL('/install.md', base).href));
  await field.evaluate(el => new Promise(resolve => requestAnimationFrame(resolve)));
  assert.equal(
    await field.evaluate((el) => el.selectionEnd - el.selectionStart),
    (await field.inputValue()).length,
  );
  assert.deepEqual(errors, []);
  console.log(
    'PASS: agent selection, copied installation prompt, hosted Markdown guide, mobile layout, and manual copy fallback.',
  );
} finally {
  await browser.close();
}
