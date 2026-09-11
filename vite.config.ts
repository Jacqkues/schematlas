import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vitest/config';
export default defineConfig({
  plugins: [sveltekit()],
  server: { port: 1420, strictPort: true },
  clearScreen: false,
  test: { include: ['src/**/*.test.ts'] },
});
