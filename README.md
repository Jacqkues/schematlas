# Schematlas

Living documentation for a code repository. Open a repository, and your local coding agent — or you — draw any kind of diagram or write docs that summarize it: architecture maps, flows, data models, module graphs. Everything is stored as files in the repository's `.schematlas/` folder, every node can point at the code it describes, and teammates see each other's presence, comments and changes live through a branch of the repository's own git remote. Database schemas and OpenAPI definitions are one more kind of map. Built with **Tauri 2, Rust, and Leptos 0.8 (WebAssembly)**.

Licensed under [Apache 2.0](LICENSE).

## Where your credentials go

Nowhere but memory. A database password reaches a Rust process that inspects the
catalog and is never written to disk, never logged, and never leaves the machine.
`ConnectionRequest`, the type that carries it, deliberately implements neither
`Serialize` nor `Debug`, so there is no code path that can serialize it into the
workspace database or print it into an error. The same holds for the API
authentication headers you configure: only the base URL is saved.

The cost is deliberate. Restarting Schematlas means reconnecting a source before
you can refresh or query it; the saved map stays browsable offline in the
meantime. Exported maps carry structure only, never credentials or row data.

What _is_ saved locally, unencrypted, is the schema itself — table, column, and
endpoint names, with their descriptions. Those can be sensitive on their own.
[Local storage and privacy](#local-storage-and-privacy) lists every file involved.

## Features

- **Repository atlas.** A project points at a repository. Diagrams live in `.schematlas/diagrams/<name>.json` and docs in `.schematlas/docs/<name>.md`, versioned and reviewed with the code. Edits made by an agent's own file tools, an editor or `git pull` appear in the open window.
- **Any diagram.** Nodes have a free kind (service, store, queue, actor…), a Markdown description, tags and `path:line` code references you can preview and open in your editor. Draw by double-clicking, connect nodes with the connect tool, group them, undo with ⌘Z, or import a Mermaid flowchart. Unplaced diagrams are laid out automatically.
- **Agent-written documentation.** The local agent gets tools to list, read, propose and edit diagrams and docs. Every write is validated first and shown as a reviewable change — added, changed and removed nodes, or a Markdown diff — and is written only after you allow it and only if the file has not changed meanwhile.
- **Team sync through git.** Opt in per repository and Schematlas shares presence ("who is looking at which diagram"), node comments, chat and change notes through a `schematlas-sync` branch on your existing remote. No server or account; see [Team sync](#team-sync).
- Projects containing multiple database connections and imported APIs.
- PostgreSQL, MySQL/MariaDB, SQLite, and SQL Server metadata inspection, including multiple schemas and cross-schema foreign keys.
- OpenAPI 3.x and Swagger 2.0 import from JSON or YAML, with endpoint and model nodes.
- A dark or light graph workspace with search, namespace filters, related-table highlighting, and cardinality labels. Select a table, then use its eye button to inspect details.
- Named, colored domain groups that move with their member tables. Automatic layout runs in a worker, organizes domains, and packs disconnected components. Canvas changes are saved, with undo for the previous edit.
- A resizable chat panel for local ACP agent sessions, installed-agent discovery, sanitized Markdown replies, and progress feedback. Tool calls show whether they are running, done, or failed, and the next question can be written while the agent is still working. Reconnecting resumes the previous conversation when the agent can replay it.
- Project-scoped agent tools for schema inspection, canvas editing, and reviewed SQL or HTTP execution.

Tables are reachable with the keyboard: Tab moves between them, Enter or Space selects one, Enter again opens its inspector, and arrow keys move it by 10 pixels or 50 with Shift. At overview zoom, column text is simplified to reduce rendering work. Highlighted edges remain behind opaque table cards. Group navigation focuses a domain without changing saved positions.

## Website

The public landing page and interactive sample workspace are in [`website/`](website/README.md).
Run its commands from that directory. Prebuilt demo assets are included; updating the
Leptos demo also requires Rust and Trunk. The live site is hosted through Sites,
separately from the desktop release workflow.

## Build and run

Install Node.js 24+, npm, Rust 1.95+, and your platform’s [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS, install Xcode Command Line Tools. CI builds and tests native packages on macOS, Windows, and Linux.

```sh
git clone https://github.com/Jacqkues/schematlas.git
cd schematlas
npm ci
rustup target add wasm32-unknown-unknown
cargo install trunk --version 0.21.14 --locked
npm run desktop
```

Build the macOS application:

```sh
npm run desktop:build
```

Open `src-tauri/target/release/bundle/macos/Schematlas.app`. The local macOS bundle is ad-hoc signed. Download CI-built installers from [Releases](https://github.com/Jacqkues/schematlas/releases); these builds are not Apple-notarized or Windows publisher-signed, so macOS reports that it cannot check the app and Windows SmartScreen warns before running the installer. [Installing on macOS](docs/macos-releases.md) explains how to open it anyway, and what it would take to publish signed builds instead.

`npm run dev` starts a visibly labeled browser preview with local example data. Database connections and file imports use the desktop runtime.

## First project

The sidebar is a file explorer: expand folders to browse diagrams, documents, database schemas, and APIs. Use **New folder** to add folders, folder options to rename or nest them, and an item’s **Move** control to organize it. New diagrams and documents include a location picker. Each project’s `.schematlas/toc.yaml` defines the titles, order, and nesting; documentation files live inside `.schematlas/`. New Markdown files follow the chosen folder path. Moving an existing entry changes navigation while preserving its file path. Existing local organization is offered as a migration preview until you save the TOC. See [the TOC format](docs/toc.md).

1. Create a project and **Open repository**. The overview is the starting point for maps and documentation.
2. Ask a question, or choose **Map the architecture**, **Explain a flow**, or **Write an overview**. This prepares a request in **Ask your agent**; choose an installed agent directly in the chat panel and send when ready. **Custom agent…** is available in the agent picker.
3. Review the agent’s proposed changes. Diagrams and documents are saved beside the code, under `.schematlas/`.
4. Open a diagram. Search for a component to bring it into view, read its explanation and source evidence, and follow its connections. **Explain with agent** prepares a question about that component. **Edit details** reveals its editable properties.
5. Double-click the canvas to add a node, drag from a node’s handle to connect it, or right-click for more actions. Use **Auto layout**, **Fit diagram to screen**, and undo/redo to organize the map. Collapse the sidebar for more canvas space.

You can also connect a database or import an OpenAPI file (`.json`, `.yaml`, or `.yml`). Use **Schemas** to choose namespaces, **Groups** to organize a domain, and the selected table’s eye button to inspect it. Credentials can be entered as individual fields or as a connection string; SQLite asks for a file.

**Explore an example project** opens a separate editable demo. The browser preview uses sample data; repositories and agent sessions require the desktop app. Unsent agent requests stay in memory while you open and close the panel, and clear when you switch projects. `examples/` also contains an empty six-table SQLite database, a sample OpenAPI definition, and synthetic schema/layout exports.

## Database support

| Engine          | Connection                                                                    | Scope                                                          |
| --------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------- |
| PostgreSQL      | `postgresql://user:password@host:5432/database?sslmode=require`               | Visible user schemas in the connected database                 |
| MySQL / MariaDB | `mysql://user:password@host:3306/database?ssl-mode=REQUIRED`                  | Accessible non-system databases                                |
| SQLite          | Select an existing database file                                              | The file's `main` schema; attached databases are not inspected |
| SQL Server      | `Server=tcp:host,1433;Database=app;User ID=user;Password=secret;Encrypt=true` | Visible user schemas; SQL authentication through Tiberius      |

A SQL Server connection string is ASCII only, and a value containing `;`, `=`, or `{` is wrapped in braces. Because that format offers no way to escape a closing brace, a password containing `}` cannot be expressed at all; the dialog says so rather than building a string the driver would misread.

The connect dialog builds the connection string from separate fields, so credentials are percent-encoded and TLS options are spelled the way the engine expects. It shows the string it will use with the password masked. **Connection string** takes a string you already have, unchanged. The table below shows what that string looks like for each engine.

Catalog queries inspect tables, views, columns, primary keys, foreign keys, and complete unconditional unique indexes. Available metadata depends on the database engine and account permissions. SQLite inspection opens the file read-only. Connections time out after 30 seconds. TLS and certificate verification follow the connection options.

MySQL URLs must include a username and database name. The fields mode encodes both for you; typing the URL yourself means percent-encoding special characters in the username and password (for example, `@` as `%40` and `#` as `%23`). Schematlas accepts `ssl-mode`, `sslmode`, and Connector/J-style `sslMode`, plus `useSSL`, `requireSSL`, and `verifyServerCertificate`. An explicit SSL mode takes precedence over the legacy flags. `useSSL=true&requireSSL=true&verifyServerCertificate=false` maps to `ssl-mode=REQUIRED`: encryption is required, but the server certificate is not verified. Use `VERIFY_IDENTITY` with an appropriate trusted CA when server identity verification is required.

Cardinality is derived from foreign keys, nullability, and available uniqueness metadata. Composite foreign keys appear as one relationship. `?` means the saved metadata does not establish a maximum; refresh or reconnect an older source to inspect unique keys. Conditional and expression indexes are not interpreted as whole-column unique keys. OpenAPI references do not receive database cardinality.

SQLx and Tiberius provide native catalog access and SQL execution. DataFusion is not included: it does not replace dialect-specific catalog discovery. Additional database engines require a connector implementing `SchemaConnector`.

## OpenAPI import

Import OpenAPI 3.x or Swagger 2.0 definitions, as JSON or YAML, up to 20 MB. The graph includes operations, reusable schemas, local references, shared request/response components, inherited parameters, arrays, and composed types. Recursive references remain finite graphs. External references are reported but never fetched automatically.

OpenAPI auto layout aligns each route with its request/response models in rows, ordered by tag (or an explicit group), path, and method. Nested models follow in columns to the right; models shared across route domains and unused models have separate rows below. Direct models stay beside their routes even when other models reference them. References use right-angle connectors whose attachment sides follow node placement. New imports use this layout automatically; click **Auto layout** to apply it to an existing saved map. Manual positions remain saved until you arrange the map again.

YAML is read with the YAML 1.2 core schema that OpenAPI 3.1 requires, so `true` and `false` are the only booleans and an `enum: [YES, NO]` stays a pair of strings. An unquoted `openapi: 3.0` or `swagger: 2.0` is a number rather than the string the specification asks for; both spellings are accepted. Duplicate mapping keys are reported instead of silently resolved. Because anchors and aliases let a few kilobytes expand into gigabytes of nodes, a YAML document is parsed under an explicit budget on nodes, events, depth, and anchor expansion; an expansion bomb is refused rather than parsed.

This is a structural explorer, not a complete OpenAPI validator. Remote multi-file resolution, top-level webhooks, and callback expansion are not implemented. Graphs are limited to 5,000 nodes and 20,000 relationships.

## Local coding agents

Schematlas hosts **Agent Client Protocol (ACP)** sessions over standard input/output. The chat panel shows installed Claude Code, Codex, Gemini CLI, and OpenCode harnesses. Selecting an available harness connects immediately in the current project folder. Discovery looks for known ACP launch commands without running them; the ACP handshake checks compatibility when connecting. Claude Code and Codex appear once each, with their adapters preferred over standalone CLIs. Plain interactive CLIs can require a separate ACP adapter; discovery does not install software or authenticate providers.

### Turn outcomes

A turn that hits a token or step limit produces an answer that simply stops, and a refused turn produces a plausible-looking reply; neither is distinguishable from success by reading the transcript. The panel therefore states the `stopReason` when a turn ends for any reason other than finishing. A refusal says so explicitly, because the specification notes that the prompt and everything after it are dropped from what the agent sees next — which changes what a follow-up question means.

Agent messages can carry images, linked files, and embedded resources as well as text. Anything that is not text is named rather than rendered, so a message never appears blank. Tool results are kept and shown under the tool's title; a file diff and terminal output are named rather than inlined.

### Resuming a conversation

Closing the app ends the agent process, but not the conversation: the agent keeps it. Each project remembers the session identifier its agent returned and a harness routing fingerprint. Only the matching harness can receive that identifier; switching harnesses starts a fresh conversation. Reconnecting calls `session/load` when the agent advertises the `loadSession` capability. The agent replays the conversation as ordinary session updates, so the panel fills with the real transcript and, more importantly, the agent resumes with the context it had rather than an empty window.

Agents that cannot replay a session, and identifiers an agent no longer recognises, fall back to a new session; a stale identifier is then forgotten rather than retried. **New conversation** abandons the current one and starts an empty session against the connected agent.

Schematlas never stores the messages themselves. The identifier is a handle; the transcript stays wherever the agent keeps it, under that agent's own retention rules.

The app supplies the agent with a project-scoped **MCP** tool bridge. Sources, tables, views, models, and endpoints are referenced by name (`orders`, `main.orders`, `Pet`, `GET /pets/{id}`); internal ids also work. Results come back as compact plain text rather than JSON, to keep the agent's context small.

| Tool             | Purpose                                                                                  |
| ---------------- | ---------------------------------------------------------------------------------------- |
| `list_sources`   | List project databases and imported APIs                                                 |
| `search_schema`  | Find tables, models, endpoints, and columns matching a term across every source          |
| `describe_table` | Show one entity's columns, keys, unique constraints, and foreign keys in both directions |
| `get_schema`     | Read a whole source as compact DDL, paginated and filterable by namespace                |
| `find_join_path` | Return the shortest foreign-key path between two tables, with a SQL skeleton             |
| `table_stats`    | Report row count and storage size for one table or view                                  |
| `sample_rows`    | Read the first rows of one table or view, unfiltered                                     |
| `explain_sql`    | Return the execution plan for one SELECT without running it                              |
| `query_sql`      | Execute one SQL statement, writes included                                               |
| `request_http`   | Call a documented OpenAPI operation on its configured connection                         |
| `get_canvas`     | Inspect node positions and group overlays                                                |
| `move_nodes`     | Move nodes to absolute canvas coordinates                                                |
| `create_group`   | Create or update a named, colored group                                                  |
| `remove_group`   | Remove a group overlay, keeping its nodes                                                |

Canvas edits save immediately and answer with a one-line confirmation. `table_stats`, `sample_rows`, `explain_sql`, `query_sql`, and `request_http` each wait for in-app approval of the exact statement or request, including writes; for the first three Schematlas prepares the statement itself, so the reviewed text is what runs. The approval shows that statement or URL as itself, with the source it runs against and any caveat, and keeps the complete payload one click away. Execution plans are not available for SQL Server. Query results are bounded: `query_sql` and `sample_rows` return 50 rows by default and at most 200, rendered as a text table with timing. Agent filesystem and shell operations follow the agent's own permission settings; Schematlas is not an operating-system sandbox for the agent.

For transport testing without a model account, use `/usr/bin/python3` as the executable and `["/absolute/path/to/schematlas/examples/mock-acp-agent.py"]` as its arguments. The fixture is explicitly labeled **ACP test agent** and is not AI.

Compatible implementations include the [Claude ACP adapter](https://github.com/agentclientprotocol/claude-agent-acp), [OpenCode ACP](https://opencode.ai/docs/acp/), and [Gemini ACP mode](https://geminicli.com/docs/cli/acp-mode/). Installation and authentication belong to those projects.

The Claude ACP adapter bundles its own Claude Code build, which can be older than the installed `claude` CLI and rejected by newer models. When the **Claude Code** harness is used and a `claude` executable is found in the usual install locations, Schematlas passes it to the adapter through `CLAUDE_CODE_EXECUTABLE`. An existing `CLAUDE_CODE_EXECUTABLE` in the environment is left unchanged.

## Team sync

Team sync is off until you turn it on for a repository in the Team panel, which first shows the remote, its address, the branch and the name teammates will see, and only then pushes.

- Each person's app appends events to its own files on an orphan `schematlas-sync` branch (`events/<key>/<day>.jsonl`, `presence/<key>.json`) and polls the remote every 15 seconds while the window is focused (60 seconds otherwise). Concurrent pushes use `--force-with-lease`, so a lost race is rebuilt on the winner and retried; nobody's events are overwritten.
- It runs the `git` command line with plumbing only. Your HEAD, index and working tree are never touched, and your own credential helper or SSH agent authenticates. Prompts are disabled, so a missing credential fails fast with a hint to authenticate in a terminal.
- Commits are titled `schematlas sync [skip ci]`; for CI systems that do not honor that marker, add `branches-ignore: [schematlas-sync]`. Events past 14 days are pruned, and the branch is squashed after 500 commits.
- Everyone with access to the repository can read the branch. Your git email is never written, only a key derived from it. Remote addresses are stored without credentials, and if a remote starts pointing elsewhere, sync pauses until you confirm again.

## Local storage and privacy

- Project snapshots, layouts, groups, and working-directory preferences are saved locally. The last ACP executable and arguments you chose are kept in local storage as a picker preference and to prefill **Custom agent…**. On macOS the existing storage location is `~/Library/Application Support/local.schema-atlas.desktop/workspace.sqlite`.
- The native identifier, browser-preview storage key, and internal bridge identifiers retain their original names for compatibility with existing installations.
- Connection strings and API authentication headers remain in Rust process memory and are never persisted or logged, as [Where your credentials go](#where-your-credentials-go) describes. Restarting requires reconnecting before refreshing or querying a database. Saved schemas remain available offline.
- Chat transcripts are never copied into the workspace. Schematlas stores the agent's session identifier and a harness routing fingerprint per project and asks the agent to replay the conversation on reconnect, so what is written stays where the agent already keeps it.
- No built-in telemetry, cloud storage, external font loading, or automatic remote-reference fetching.
- Schema metadata and descriptions can be sensitive; local project storage is not encrypted.
- Your chosen coding agent may send prompts, schemas, and approved query results to its model provider according to its configuration.
- Diagrams and docs are files in your repository, not in the workspace database. Team sync events are on the repository's `schematlas-sync` branch; an unsent outbox is kept in `.git/schematlas/`.
- Deleting a project removes its local maps, not its working-directory files or database. Exported maps exclude connection credentials and row data.

## Source structure

```text
ui/src/components/       Modular Leptos components
  agent/                  ACP chat, discovery, activity, reviews, Markdown
  atlas.rs                Repository overview, diagram editor, inspector and docs
  team.rs                 Team sync setup, presence and activity feed
  dialogs.rs              Accessible native HTML dialogs
  graph/                  DOM cards, SVG edges, groups and viewport culling
ui/src/api.rs             Typed Tauri IPC boundary and event cleanup
ui/src/appearance.rs      Persistent dark/light appearance
ui/src/state.rs           Workspace state and selection
ui/src/smart_layout.rs    Pure Rust relationship-aware domain layout
ui/src/diagram_layout.rs  Diagram card geometry and placement
ui/src/bin/layout-worker.rs  Isolated background layout computation
ui/app.css                Tailwind theme tokens and shared utilities
src-tauri/src/
  domain.rs               Project/schema contracts and validation
  diagram.rs              Diagram file format, validation and change summaries
  repo.rs                 Safe `.schematlas/` file access with compare-and-swap writes
  atlas.rs                Project-level atlas operations and file watching
  mermaid.rs              Mermaid flowchart import
  sync/                   Team channel over the `schematlas-sync` git branch
  service.rs              Workspace use cases
  repository.rs           SQLite project persistence
  connectors/             Database-specific metadata adapters
  agents/                 ACP sessions, project-scoped MCP bridge and atlas tools
  execution/              Reviewed SQL and HTTP execution
  canvas.rs               Validated layout and group edits
  openapi.rs              Bounded OpenAPI parser
  commands.rs             Tauri IPC commands
examples/                 Synthetic fixtures and deterministic test agent
```

## Checks

```sh
npm run check
npm run check:tests
npm test
cargo fmt --manifest-path ui/Cargo.toml --check
cargo clippy --manifest-path ui/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path ui/Cargo.toml --target wasm32-unknown-unknown -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
npm run test:e2e
```

The browser specifications use installed Google Chrome locally and Playwright Chromium in CI. Native bridge browser tests use a deterministic mock; backend tests and the packaged transport check validate the Rust side. Unit tests cover layout bounds, domains, relationship cardinality, Markdown sanitization, SQLite metadata, persistence, ACP sessions, tool authorization, and cancellation. Some database integration tests are ignored by default because they require isolated disposable databases. Never point these tests at a production database.

After a desktop build, check the packaged MCP transport:

```sh
python3 scripts/test-mcp-stdio.py src-tauri/target/release/schematlas
```

SQL Server has compile-time coverage but has not been exercised against a live server. Native performance has been checked interactively; the project does not claim a frame-rate guarantee.

## License

Schematlas source is licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE) for attribution. Bundled fonts and dependencies retain their respective licenses.

## CI and downloadable releases

[Builds](https://github.com/Jacqkues/schematlas/actions/workflows/build.yml) run on pull requests and pushes to `main`. The matrix checks Leptos on the WASM target, frontend unit tests, browser-test types, Rust formatting, strict Clippy, installer builds, and the compiled MCP tool transport. Linux also runs the Leptos browser regression suite. CI installer artifacts remain downloadable from each run for 14 days.

To publish a release, update the version in `package.json`, both root entries in `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json`. Commit, then push a matching stable version tag:

```sh
git tag v0.4.0
git push origin main v0.4.0
```

The workflow verifies version consistency and builds these downloads:

| Platform | Architecture                        | Formats                 |
| -------- | ----------------------------------- | ----------------------- |
| macOS    | Apple Silicon and Intel, separately | `.dmg`                  |
| Windows  | x64                                 | NSIS `.exe`, WiX `.msi` |
| Linux    | x64                                 | `.AppImage`, `.deb`     |

Only after all four build jobs succeed does a separate job create a draft release, upload all six installers plus `SHA256SUMS`, and publish it. Failed matrix jobs cannot publish a partial release. Retry failed jobs from Actions; already published releases are not overwritten. Manually dispatching on `main` builds artifacts without publishing; a version tag triggers publication.

Downloads appear on the [Releases page](https://github.com/Jacqkues/schematlas/releases). This uses GitHub's built-in workflow token and requires no personal access token. macOS builds use ad-hoc signing; Apple notarization and Windows publisher signing are not configured, so installation may require an OS confirmation. Automatic in-app updates are not part of this pipeline.
