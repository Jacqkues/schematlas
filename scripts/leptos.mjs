import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const ui = fileURLToPath(new URL('../ui/', import.meta.url));
const [command, ...args] = process.argv.slice(2);
function buildStyles() {
  return spawnSync(
    process.execPath,
    [
      fileURLToPath(new URL('../node_modules/@tailwindcss/cli/dist/index.mjs', import.meta.url)),
      '-i',
      'app.css',
      '-o',
      'public/app.css',
      '--minify',
    ],
    { cwd: ui, stdio: 'inherit' },
  );
}

let result;
if (command === 'css') {
  result = buildStyles();
} else if (command === 'serve' || command === 'build') {
  if (command === 'serve') {
    // Trunk canonicalizes every watch.ignore entry before running pre-build hooks.
    // A fresh checkout has none of these generated paths yet.
    for (const directory of ['dist', 'dist-dev'])
      mkdirSync(`${ui}${directory}`, { recursive: true });
    if (!existsSync(`${ui}public/app.css`)) {
      const styles = buildStyles();
      if (styles.error || styles.status !== 0) {
        if (styles.error) console.error(styles.error.message);
        process.exit(styles.status ?? 1);
      }
    }
  }
  const env = { ...process.env };
  // A native Tauri target directory must not redirect frontend WASM artifacts.
  env.CARGO_TARGET_DIR = process.env.SCHEMATLAS_UI_TARGET_DIR ?? `${ui}target`;
  if ('NO_COLOR' in env) env.NO_COLOR = 'true';
  result = spawnSync(
    'trunk',
    [command, ...(command === 'serve' ? ['--dist', 'dist-dev'] : []), ...args],
    {
      cwd: ui,
      env,
      stdio: 'inherit',
    },
  );
  if (result.error?.code === 'ENOENT') {
    console.error('Trunk is required. Run: cargo install trunk --version 0.21.14 --locked');
  }
} else {
  console.error('Usage: node scripts/leptos.mjs <serve|build|css> [Trunk arguments]');
  process.exit(1);
}
if (result.error) console.error(result.error.message);
process.exit(result.status ?? 1);
