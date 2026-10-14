# schematlas-parser

Reads the diagram files people and agents already write, into one graph model, and edits them
back surgically. Schematlas's own information (links to code, notes, kinds, tags) lives inside the
same files in a form the original tools ignore, so a repository stays useful without the app.

| Format | Read | Edit | Schematlas annotations |
| --- | --- | --- | --- |
| Mermaid flowchart (`.mmd`, ```` ```mermaid ```` in Markdown) | yes, via [merman](https://github.com/Latias94/merman) | nodes, edges, labels, removal — as minimal text patches | `%% @schematlas node api code=src/api.rs:12-80 note="…"` |
| Mermaid class, ER, state | yes | annotations | same |
| Other Mermaid families | recognised, refused by name | — | — |
| draw.io (`.drawio`, plain or compressed, multi-page) | yes, absolute positions | everything above plus moving nodes and containers | the cell's own custom properties (`schematlas-code`, …), shown under *Edit Data* |
| `.drawio.svg` exports | yes | no (edit the `.drawio`) | — |
| DBML (via [dbml-rs](https://github.com/apskhem/dbml-rs)) | yes, aliases and schemas resolved | canonical writing, diff, and merging a live database into the file while keeping notes and comments | `// @schematlas node public.users code=…` |

Every edit is checked: the edited text is read back and must equal the model's own view of the
edit (`model::apply`), or the edit is refused and the file is untouched.

Pure functions only (no I/O); builds for native targets and `wasm32-unknown-unknown`.

## Tests

```sh
cargo test                                          # unit, fixtures, snapshots, invariants
SCHEMATLAS_GEN_SEEDS=1500 cargo test --release      # longer soak of the generated suites
cargo clippy --all-targets -- -D warnings
cargo clippy --target wasm32-unknown-unknown -- -D warnings
```

Snapshots are reviewed with `cargo insta review`.
