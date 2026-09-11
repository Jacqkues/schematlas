import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: 'tests',
  testMatch: '*.spec.ts',
  use: {
    baseURL: 'http://127.0.0.1:1420',
    channel: 'chrome',
    viewport: { width: 1440, height: 940 },
  },
  webServer: { command: 'npm run dev', url: 'http://127.0.0.1:1420', reuseExistingServer: true },
  reporter: 'list',
});
