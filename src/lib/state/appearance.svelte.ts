import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

export type Theme = 'dark' | 'light';
export const appearance = $state<{ theme: Theme }>({ theme: 'dark' });

function apply(theme: Theme) {
  appearance.theme = theme;
  document.documentElement.dataset.theme = theme;
  document.querySelector('meta[name="color-scheme"]')?.setAttribute('content', theme);
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute('content', theme === 'light' ? '#f7f8fa' : '#070809');
  if (isTauri()) {
    const window = getCurrentWindow();
    void Promise.all([
      window.setTheme(theme),
      window.setBackgroundColor(theme === 'light' ? '#f7f8fa' : '#070809'),
    ]).catch(() => console.warn('Native window appearance could not be updated.'));
  }
}

export function setTheme(theme: Theme) {
  apply(theme);
  try {
    localStorage.setItem('schematlas.theme', theme);
  } catch {
    /* Session-only preference. */
  }
}

export function initializeAppearance() {
  apply(document.documentElement.dataset.theme === 'light' ? 'light' : 'dark');
  const sync = (event: StorageEvent) => {
    if (event.key === 'schematlas.theme' || event.key === null)
      apply(event.newValue === 'light' ? 'light' : 'dark');
  };
  window.addEventListener('storage', sync);
  return () => window.removeEventListener('storage', sync);
}
