# Schematlas

Living documentation for your repository: architecture diagrams, a wiki and database schemas, written with your local coding agent and stored as files in your repository.

Built with Tauri 2, Rust and Leptos. Licensed under [Apache 2.0](LICENSE).

## Download

Installers for macOS, Windows and Linux are on the [Releases page](https://github.com/Jacqkues/schematlas/releases). The builds are not notarized or publisher-signed, so your OS asks for confirmation the first time; see [Installing on macOS](docs/macos-releases.md).

## Build from source

Requirements:

- Node.js 24+ and npm
- Rust 1.95+
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform (on macOS, the Xcode Command Line Tools)

```sh
git clone https://github.com/Jacqkues/schematlas.git
cd schematlas
npm ci
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
```

On Intel macOS, install Binaryen so Trunk uses its working `wasm-opt` from PATH
instead of the bundled Binaryen 123 download, whose macOS Intel library fails to load:

```sh
brew install binaryen
wasm-opt --version
```

Run the desktop app in development:

```sh
npm run desktop
```

Build installers (output in `src-tauri/target/release/bundle/`):

```sh
npm run desktop:build
```

`npm run dev` starts a browser preview with example data. Repositories, databases and agents need the desktop app.

## Tests

```sh
npm run check
npm run check:tests
npm test
npm run test:e2e
cargo test --manifest-path src-tauri/Cargo.toml
cargo +1.95.0 clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo +1.95.0 clippy --locked --manifest-path ui/Cargo.toml --all-targets -- -D warnings
```

Database integration tests are ignored by default; run them only against disposable databases.

## Local storage and privacy

- Diagrams and docs are files in your repository, under `.schematlas/`.
- Projects and schema snapshots are stored locally and unencrypted (on macOS, `~/Library/Application Support/local.schema-atlas.desktop/`).
- Connection strings and API credentials stay in memory and are never saved or logged.
- Agent conversations stay with the agent; Schematlas keeps only a session id to resume them.
- No telemetry. Your coding agent may send prompts and schemas to its model provider, according to its own settings.

## Releases

Bump the version in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and `src-tauri/tauri.conf.json`, then push a matching tag:

```sh
git tag v0.4.0
git push origin main v0.4.0
```

CI builds the installers and publishes the release.

## Website

The landing page lives in [`website/`](website/README.md).
