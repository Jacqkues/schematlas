# Working on Schematlas

- Keep project/source invariants in domain/service code, catalog SQL in connectors, and Tauri commands thin.
- Never log or persist connection strings. ConnectionRequest must not derive Serialize or Debug.
- Keep schema inspection read-only. Agent SQL and HTTP execution is separately authorized through project-scoped, reviewed tool requests. Never execute a SQL/HTTP tool call before its in-app approval. Do not fetch external OpenAPI references.
- MCP tool results are plain text sized for a model's context, and per-session guidance belongs in the MCP `instructions` rather than per-turn prompts or per-tool descriptions. Tools take human-readable references (`main.orders`, `GET /pets/{id}`) and resolve them server-side.
- The frontend is Leptos 0.8 in ui/. Keep pure graph algorithms in the UI library and run layout in its worker binary. Native dialog.showModal provides focus management.
- Style with Tailwind utilities in markup. Leptos theme tokens and shared `@utility` classes live in ui/app.css; keep motion reduced-motion aware.
- Browser support: current macOS WKWebView; use broadly supported HTML/CSS. Provide fallbacks for newer APIs.
- Keep browser preview visibly labeled and distinct from native functionality.
- Run npm run check, npm run check:tests, npm test, npm run test:e2e, and formatting/strict Clippy for ui/Cargo.toml on native and wasm32-unknown-unknown. For backend changes, also run cargo test and strict Clippy with src-tauri/Cargo.toml.
- Use disposable databases for ignored integration tests. Never point them at user databases.
