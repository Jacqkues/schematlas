import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vitest/config';
export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: { port: 1420, strictPort: true },
  clearScreen: false,
  test: { include: ['src/**/*.test.ts'] },
});
