# Repository documentation TOC

`.schematlas/toc.yaml` is the source of truth for documentation navigation. Its ordered `items` arrays determine the sidebar order; no alphabetical sorting or fixed content categories are applied.

```yaml
version: 1
items:
  - title: Overview
    file: overview.md
  - title: Architecture
    children:
      - title: System map
        file: architecture/system.mmd
      - title: Checkout
        file: architecture/checkout.md
      - title: Embedded flow
        file: architecture/checkout.md
        block: 0
      - title: Deployment
        file: architecture/deployment.drawio
        page: 1
  - title: Data
    children:
      - title: Commerce schema
        file: data/commerce.dbml
```

Paths are relative to the repository root, with forward slashes (for example `.schematlas/wiki/overview.md` or `docs/architecture.mmd`). Absolute paths, `..`, `.git`, external references, symlinks, `.schematlas/toc.yaml` itself, and `.schematlas/layout/` are refused. A folder has `title` and `children` (including `children: []`); a file has `title` and `file`. Optional `type` is `doc`, `diagram`, `database`, or `openapi`. Optional zero-based `block` selects a Mermaid block in Markdown; `page` selects a draw.io page. Files can appear as both a Markdown document and a selected block. Duplicate references to the same file/selector are invalid.

TOC identity is separate from diagram identity. `Entry::document_id()` adapts a TOC reference into the parser integration's repository-relative `.schematlas/<file>#<part>` ID. Folders use transient IDs derived from their title ancestry; there are no machine-generated IDs in the YAML.

Existing local source references migrate as `source: <name>` with `type: database` or `openapi`. These resolve against locally connected sources by name and type; they never contain connection strings or credentials. Portable schema documentation should use a DBML file once exported. Source references that are unavailable or ambiguous on another machine remain visible with a diagnostic.

The implementation lives in `shared/toc.rs` (format and edit invariants), `src-tauri/src/toc_service.rs` (repository persistence and migration), and `ui/src/components/explorer.rs` (navigation). Parser integration owns standard-file loading and editing. UI entry dispatch resolves TOC references against the repository listing rather than decoding diagram content.

A project without a TOC shows a migration preview of its existing folder organization. Saving the structure or the first navigation edit creates `toc.yaml`; reads do not modify the repository. Subsequent changes use its content hash to reject stale writes. Invalid YAML is reported and never replaced automatically. Files omitted from the TOC remain discoverable separately and can be added explicitly.

Navigation moves change TOC nesting, preserving the target file path. Physical file moves must update their references as a separate operation. Content creation registers the new file in the TOC, while existing-file edits preserve the navigation structure.

UI edits serialize the TOC into a consistent YAML format, preserving the entries and their order but not comments or original formatting. Removing a TOC entry keeps its file. Deleting a referenced file leaves a visible missing-file entry until its reference is removed or corrected.

Agents use `read_toc` to retrieve the YAML and its hash, then `write_toc` with the complete proposed YAML and `base_hash`. Validation happens before review; the proposed diff must be approved in the app, and its hash is checked again before saving. Agent proposals are limited to 64 KB. These tools also allow a malformed TOC to be repaired without silently replacing it.

## Parser integration handoff

This change is based on `d6db9b6` on `feat/repo-atlas`. Standard-file diagram loading remains owned by the parser integration branch. Until those changes are combined, the diagram creation/loading adapter still uses the previous JSON diagram API; the TOC schema already supports the standard formats.

- Preserve `Workspace.toc` and the TOC refresh from `refresh_repo` so repository watcher events update navigation and nested document hashes.
- Adapt `known_entries`, `active`, and `open_entry` in `ui/src/components/explorer.rs` to `DiagramEntry.id/path/part`. Use `Entry::document_id()` for diagram identity, and retain Markdown-without-`block` as a document. Only offer files beneath `.schematlas/` for adding to this TOC. Keep selectors when adding embedded blocks or draw.io pages.
- In `CreateDialog`, replace the legacy diagram save with standard-file creation and register the returned path and selector using `Edit::Add`. Keep the Markdown `create_toc_doc` flow, which writes a nested document and registers it together.
- Retain the TOC service's `.schematlas/`-relative read/write/delete boundary. When adopting the new generic `Repo::read/write/delete`, the `*_atlas` methods can become validated wrappers that prefix `.schematlas/`. Keep `toc.yaml` specially allowed and keep other TOC target validation in `shared/toc.rs`.
- Adapt migration to the new listing types and use their actual paths; migrate old JSON references to the replacement standard files when converting those files. `Project.explorer` must remain read-only migration input.
- Keep recursive Markdown discovery and the path-based Markdown API. The `DocFile.slug` compatibility field holds a relative file path for new documents; old flat slugs still resolve to `docs/<slug>.md`.
- Keep `read_toc`/`write_toc` tools and their session instructions when merging the agent-tool router. Diagram writes and TOC writes remain separately reviewed changes.

Overlapping UI files are `ui/src/api.rs`, `ui/src/components/atlas.rs`, `ui/src/components/explorer.rs`, and `ui/src/preview.rs`. This change does not edit `ui/src/types.rs` or the diagram canvas.
