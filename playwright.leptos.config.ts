import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: 'tests/leptos',
  fullyParallel: true,
  timeout: 60_000,
  expect: { timeout: 15_000 },
  use: {
    baseURL: 'http://127.0.0.1:1421',
    channel: process.env.CI ? undefined : 'chrome',
    viewport: { width: 1440, height: 940 },
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'npm run dev -- --port 1421',
    url: 'http://127.0.0.1:1421',
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
  reporter: 'list',
});
