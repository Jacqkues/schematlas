# Working on Schematlas

- Keep project/source invariants in domain/service code, catalog SQL in connectors, and Tauri commands thin.
- Never log or persist connection strings. ConnectionRequest must not derive Serialize or Debug.
- Keep schema inspection read-only. Agent SQL and HTTP execution is separately authorized through project-scoped, reviewed tool requests. Never execute a SQL/HTTP tool call before its in-app approval. Do not fetch external OpenAPI references.
- Use Svelte 5 runes and typed props. Native dialog.showModal provides focus management.
- Browser support: current macOS WKWebView; use broadly supported HTML/CSS. Provide fallbacks for newer APIs.
- Keep browser preview visibly labeled and distinct from native functionality.
- Run npm run check, npm test, cargo test, and strict Clippy for relevant changes.
- Use disposable databases for ignored integration tests. Never point them at user databases.
