# Schematlas

A local desktop workspace for exploring database schemas and OpenAPI definitions, with an integrated coding-agent chat. Built with **Tauri 2, Rust, SvelteKit, Svelte 5, and SvelteFlow**.

Licensed under [Apache 2.0](LICENSE).

## Features

- Projects containing multiple database connections and imported APIs.
- PostgreSQL, MySQL/MariaDB, SQLite, and SQL Server metadata inspection, including multiple schemas and cross-schema foreign keys.
- OpenAPI 3.x and Swagger 2.0 JSON import, with endpoint and model nodes.
- A dark graph workspace with search, namespace filters, related-table highlighting, and cardinality labels. Select a table, then use its eye button to inspect details.
- Named, colored domain groups that move with their member tables. Automatic layout runs in a worker, organizes domains, and packs disconnected components. Canvas changes are saved, with undo for the previous edit.
- A resizable chat panel for local ACP agent sessions, installed-agent discovery, sanitized Markdown replies, and progress feedback.
- Project-scoped agent tools for schema inspection, canvas editing, and reviewed SQL or HTTP execution.

At overview zoom, column text is simplified to reduce rendering work. Highlighted edges remain behind opaque table cards. Group navigation focuses a domain without changing saved positions.

## Build and run

Install Node.js 24+, npm, Rust 1.95+, and your platform’s [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS, install Xcode Command Line Tools. CI builds and tests native packages on macOS, Windows, and Linux.

```sh
git clone https://github.com/Jacqkues/schematlas.git
cd schematlas
npm ci
npm run desktop
```

Build the macOS application:

```sh
npm run desktop:build
```

Open `src-tauri/target/release/bundle/macos/Schematlas.app`. The local macOS bundle is ad-hoc signed. Download CI-built installers from [Releases](https://github.com/Jacqkues/schematlas/releases); these builds are not Apple-notarized or Windows publisher-signed.

`npm run dev` starts a visibly labeled browser preview with local example data. Database connections and file imports use the desktop runtime.

## First project

1. Create a project and connect a database or import an OpenAPI JSON file.
2. Use **Schemas** to choose namespaces and optionally include linked tables.
3. Click **Auto layout** to organize the map. Use **Groups** to focus, create, edit, color, or remove a domain overlay.
4. Select a table to highlight its direct relationships; click the eye to open its inspector.
5. Open **Local agent**, select an installed ACP adapter, and connect. The app prepares a project working directory; an existing repository can be selected instead.

**Take a look around with an example project** creates a separate editable demo. `examples/` also contains an empty six-table SQLite database, a sample OpenAPI definition, and synthetic schema/layout exports.

## Database support

| Engine | Connection | Scope |
| --- | --- | --- |
| PostgreSQL | `postgresql://user:password@host:5432/database?sslmode=require` | Visible user schemas in the connected database |
| MySQL / MariaDB | `mysql://user:password@host:3306/database?ssl-mode=REQUIRED` | Accessible non-system databases |
| SQLite | Select an existing database file | The file's `main` schema; attached databases are not inspected |
| SQL Server | `Server=tcp:host,1433;Database=app;User ID=user;Password=secret;Encrypt=true` | Visible user schemas; SQL authentication through Tiberius |

Catalog queries inspect tables, views, columns, primary keys, foreign keys, and complete unconditional unique indexes. Available metadata depends on the database engine and account permissions. SQLite inspection opens the file read-only. Connections time out after 30 seconds; certificate validation is not disabled.

Cardinality is derived from foreign keys, nullability, and available uniqueness metadata. Composite foreign keys appear as one relationship. `?` means the saved metadata does not establish a maximum; refresh or reconnect an older source to inspect unique keys. Conditional and expression indexes are not interpreted as whole-column unique keys. OpenAPI references do not receive database cardinality.

SQLx and Tiberius provide native catalog access and SQL execution. DataFusion is not included: it does not replace dialect-specific catalog discovery. Additional database engines require a connector implementing `SchemaConnector`.

## OpenAPI import

Import OpenAPI 3.x or Swagger 2.0 JSON files up to 20 MB. The graph includes operations, reusable schemas, local references, shared request/response components, inherited parameters, arrays, and composed types. Recursive references remain finite graphs. External references are reported but never fetched automatically.

This is a structural explorer, not a complete OpenAPI validator. YAML, remote multi-file resolution, top-level webhooks, and callback expansion are not implemented. Graphs are limited to 5,000 nodes and 20,000 relationships.

## Local coding agents

Schematlas hosts **Agent Client Protocol (ACP)** sessions over standard input/output. Discovery looks for compatible executables without running them. Plain interactive CLIs can require a separate ACP adapter; discovery does not install software or authenticate providers.

The app supplies the agent with a project-scoped **MCP** tool bridge:

| Tool | Purpose |
| --- | --- |
| `list_sources` | List project databases and imported APIs |
| `get_schema` | Read a source's schema metadata |
| `get_canvas` | Inspect node positions and groups |
| `move_nodes` | Move nodes on the canvas |
| `create_group` | Create or update a named, colored group |
| `remove_group` | Remove a group overlay |
| `query_sql` | Execute SQL after in-app approval of the exact request |
| `request_http` | Make an API request after in-app approval |

Canvas edits save immediately. SQL and HTTP calls require individual review, including writes. Query results are bounded. Agent filesystem and shell operations follow the agent's own permission settings; Schematlas is not an operating-system sandbox for the agent.

For transport testing without a model account, use `/usr/bin/python3` as the executable and `["/absolute/path/to/schematlas/examples/mock-acp-agent.py"]` as its arguments. The fixture is explicitly labeled **ACP test agent** and is not AI.

Compatible implementations include the [Claude ACP adapter](https://github.com/agentclientprotocol/claude-agent-acp), [OpenCode ACP](https://opencode.ai/docs/acp/), and [Gemini ACP mode](https://geminicli.com/docs/cli/acp-mode/). Installation and authentication belong to those projects.

The Claude ACP adapter bundles its own Claude Code build, which can be older than the installed `claude` CLI and rejected by newer models. When the **Claude ACP** preset is used and a `claude` executable is found in the usual install locations, Schematlas passes it to the adapter through `CLAUDE_CODE_EXECUTABLE`. An existing `CLAUDE_CODE_EXECUTABLE` in the environment is left unchanged.

## Local storage and privacy

- Project snapshots, layouts, groups, and working-directory preferences are saved locally. On macOS the existing storage location is `~/Library/Application Support/local.schema-atlas.desktop/workspace.sqlite`.
- The native identifier, browser-preview storage key, and internal bridge identifiers retain their original names for compatibility with existing installations.
- Connection strings and API authentication headers remain in Rust process memory. Restarting requires reconnecting before refreshing or querying a database. Saved schemas remain available offline.
- Chat transcripts are currently memory-only and clear on restart.
- No built-in telemetry, cloud storage, external font loading, or automatic remote-reference fetching.
- Schema metadata and descriptions can be sensitive; local project storage is not encrypted.
- Your chosen coding agent may send prompts, schemas, and approved query results to its model provider according to its configuration.
- Deleting a project removes its local maps, not its working-directory files or database. Exported maps exclude connection credentials and row data.

## Source structure

```text
src/lib/components/       Modular Svelte 5 components
  agent/                  ACP chat, discovery, activity, reviews, Markdown
  dialogs/                Native accessible dialogs
  graph/                  Nodes, edges, groups, canvas interactions
src/lib/services/         IPC boundary, layout worker, graph helpers, preview
src/lib/state/            Workspace state
src-tauri/src/
  domain.rs               Project/schema contracts and validation
  service.rs              Workspace use cases
  repository.rs           SQLite project persistence
  connectors/             Database-specific metadata adapters
  agents/                 ACP sessions and project-scoped MCP bridge
  execution/              Reviewed SQL and HTTP execution
  canvas.rs               Validated layout and group edits
  openapi.rs              Bounded OpenAPI parser
  commands.rs             Tauri IPC commands
examples/                 Synthetic fixtures and deterministic test agent
```

## Checks

```sh
npm run check
npm test
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
npm run test:e2e
```

The browser specifications use an installed Google Chrome. Unit tests cover layout bounds, domains, relationship cardinality, Markdown sanitization, SQLite metadata, persistence, ACP sessions, tool authorization, and cancellation. Some database integration tests are ignored by default because they require isolated disposable databases. Never point these tests at a production database.

After a desktop build, check the packaged MCP transport:

```sh
python3 scripts/test-mcp-stdio.py src-tauri/target/release/schematlas
```

SQL Server has compile-time coverage but has not been exercised against a live server. Native performance has been checked interactively; the project does not claim a frame-rate guarantee.

## License

Schematlas source is licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE) for attribution. Bundled fonts and dependencies retain their respective licenses.

## CI and downloadable releases

[Builds](https://github.com/Jacqkues/schematlas/actions/workflows/build.yml) run on pull requests and pushes to `main`. The matrix checks Svelte, frontend tests, Rust formatting, Rust tests, strict Clippy, installer builds, and the compiled MCP tool transport. CI installer artifacts remain downloadable from each run for 14 days.

To publish a release, update the version in `package.json`, both root entries in `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Commit, then push a matching stable version tag:

```sh
git tag v0.4.0
git push origin main v0.4.0
```

The workflow verifies version consistency and builds these downloads:

| Platform | Architecture | Formats |
| --- | --- | --- |
| macOS | Apple Silicon and Intel, separately | `.dmg` |
| Windows | x64 | NSIS `.exe`, WiX `.msi` |
| Linux | x64 | `.AppImage`, `.deb` |

Only after all four build jobs succeed does a separate job create a draft release, upload all six installers plus `SHA256SUMS`, and publish it. Failed matrix jobs cannot publish a partial release. Retry failed jobs from Actions; already published releases are not overwritten. Manually dispatching on `main` builds artifacts without publishing; a version tag triggers publication.

Downloads appear on the [Releases page](https://github.com/Jacqkues/schematlas/releases). This uses GitHub's built-in workflow token and requires no personal access token. macOS builds use ad-hoc signing; Apple notarization and Windows publisher signing are not configured, so installation may require an OS confirmation. Automatic in-app updates are not part of this pipeline.
