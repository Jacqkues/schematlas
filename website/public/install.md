# Install Schematlas with your coding agent

Schematlas is a native desktop app for exploring a repository's diagrams, wiki,
and database schemas. Build the desktop app from the official source repository:
https://github.com/Jacqkues/schematlas

This guide works with Codex, Claude Code, and other agents with terminal access.
Run commands on the computer where the app will be used, in a desktop session.
A remote coding sandbox can compile artifacts, but cannot install or open a GUI
on a different computer.

## 1. Choose the source

Find the latest stable release at:
https://github.com/Jacqkues/schematlas/releases/latest

Resolve its actual tag, then replace `<stable-tag>` below with that tag.
Clone into a new directory outside the user's existing projects:

```sh
git clone --branch <stable-tag> https://github.com/Jacqkues/schematlas.git schematlas
cd schematlas
git rev-parse HEAD
```

Read this checkout's `README.md`, `AGENTS.md`, `package.json`,
`src-tauri/Cargo.toml`, and `.github/workflows/build.yml` before building.
They take precedence if requirements have changed since this guide was published.
If the user wants unreleased changes, clone the default branch instead and report
its commit. Reuse an existing checkout only with the user's agreement; preserve
its changes and avoid resetting it.

## 2. Check prerequisites

Use existing installations when compatible. Current requirements:

- Git.
- Node.js 24 or newer, with npm.
- Rust 1.95 or newer, installed through rustup; CI uses Rust 1.95.0.
- Trunk 0.21.14 and the `wasm32-unknown-unknown` Rust target.
- Native Tauri 2 prerequisites for the host OS.

Check `git --version`, `node --version`, `npm --version`, `rustup --version`,
`rustc --version`, and `trunk --version`. Install missing tools using their
maintainers' official instructions and the user's normal permission flow.

### macOS

Use Xcode Command Line Tools for desktop builds. Check with `xcode-select -p`;
if missing, run `xcode-select --install` and wait for the installer to complete.
Build natively on Apple silicon or Intel to get the matching architecture.

### Windows

Use native Windows with the MSVC Rust toolchain, Microsoft C++ Build Tools
("Desktop development with C++" workload), and Microsoft Edge WebView2 Runtime.
WSL builds produce Linux apps, not Windows apps.

### Linux

Use a desktop session and your distribution's Tauri 2 system packages.
For Debian/Ubuntu, the development packages include WebKitGTK 4.1, a C/C++
build toolchain, OpenSSL, Ayatana AppIndicator, and librsvg. Packaging may also
require patchelf and FUSE support. Follow the distribution-specific list here:

https://v2.tauri.app/start/prerequisites/

The same official page covers the macOS and Windows prerequisites.

## 3. Prepare the checkout

Run the following from the **repository root**, not `website/`.
These commands select Rust only for this checkout and do not change the global
default toolchain:

```sh
rustup toolchain install 1.95.0 --profile minimal
rustup override set 1.95.0
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
npm ci
```

Ensure the rustup/Cargo binaries are on PATH (`~/.cargo/bin` on macOS/Linux,
`%USERPROFILE%\.cargo\bin` on Windows). Reopen the terminal if installers changed
PATH. If Trunk 0.21.14 is already available, skip its installation.

## 4. Compile the native app

```sh
npm run desktop:build -- -- --locked
```

This builds the Leptos WASM frontend and Tauri desktop app together, including
installers. The first build downloads dependencies and can take several minutes.
Do not substitute `npm run dev`: that is a browser preview without native
repository, database, or agent access.

Output is normally in `src-tauri/target/release/bundle/`. If an explicit Rust
`--target` is used, look in `src-tauri/target/<target>/release/bundle/` instead;
a custom `CARGO_TARGET_DIR` also changes the location.

If Windows MSI packaging fails because its optional VBScript support is missing,
build just the EXE installer:

```sh
npm run desktop:build -- --bundles nsis -- --locked
```

If only macOS DMG creation fails after the app has compiled, build an app bundle:

```sh
npm run desktop:build -- --bundles app -- --locked
```

Do not claim success if compilation or packaging failed. Report the failing
command and error, fix the cause, then retry the affected step.

## 5. Install and verify

- **macOS:** install the generated `Schematlas.app` from the app bundle or DMG
  into the user's Applications folder (for example `~/Applications/`), then open
  it. Ask before replacing an existing installation. Locally built apps are not
  notarized; use the normal macOS per-app confirmation if needed. Instructions:
  https://github.com/Jacqkues/schematlas/blob/main/docs/macos-releases.md
- **Windows:** open the generated NSIS `.exe` or MSI installer and complete its
  installation UI, then launch Schematlas.
- **Linux:** use the generated AppImage (make that file executable before
  launching), or install the generated `.deb` with the OS package manager.

Verify the native Schematlas window opens. No database connection, credentials,
or agent login is needed merely to install or open the app. Report the release
tag, commit SHA, architecture, build output, installation path, and whether launch
was verified. If the environment cannot display a GUI, report that limit.

## Optional: development mode

From the repository root after the same setup:

```sh
npm run desktop
```

This runs the actual desktop app with a development server. Keep it distinct
from a packaged installation.
