---
title: How it works
description: Schematlas reads the diagrams and docs in your repository, adds what those formats cannot hold, and writes every change back to the same files.
---

## The repository is the source of truth

A Schematlas project points at a folder. Everything it shows comes from files in that folder, and every change you or your agent make is written back to them. There is no export step and no copy in a database: open the same files in VS Code, on GitHub or in draw.io and you see the same thing.

| What | Where | Format |
| --- | --- | --- |
| Diagrams you already have | anywhere in the repository | Mermaid (`.mmd`, `.mermaid`), draw.io (`.drawio`, `.dio`), DBML (`.dbml`), Mermaid blocks in Markdown |
| Diagrams made in Schematlas | `.schematlas/diagrams/` | Mermaid |
| Wiki pages | `.schematlas/wiki/` and `.schematlas/` | Markdown |
| Saved database schemas | `.schematlas/schema/` | DBML |
| Navigation | `.schematlas/toc.yaml` | YAML |
| Arrangements you made by dragging | `.schematlas/layout/` | JSON |

## Adding data to existing formats

Mermaid, draw.io and DBML describe boxes and arrows. Schematlas needs a little more: which code a node stands for, a note, a kind, a color, the commit it was last checked against. It stores that inside each file, in a place the original tool ignores:

- **Mermaid**: comment lines, `%% @schematlas node api code=src/api.rs:12-80`.
- **draw.io**: the cell's own custom properties, `schematlas-code`, `schematlas-note`, which draw.io shows under *Edit Data*.
- **DBML**: comment lines, `// @schematlas node public.orders note="Customer orders"`.

The file still renders in Mermaid Live, draw.io or dbdiagram.io, and diffs stay readable. See [Annotations](/docs/annotations) for every key.

## Edits keep your text

Schematlas edits a file by patching the part that changed, not by regenerating it. Your formatting, comments, ordering and styling survive. A node you rename changes one label; a note you add is one comment line.

Every write is a compare-and-swap on the file's content hash: if the file changed on disk since Schematlas read it — an editor, a `git pull`, your agent's own tools — the write is refused and the latest version is shown instead. Changes made outside the app appear in the open window.

## Layout

Diagrams without saved positions are laid out automatically with the same layered algorithm Mermaid uses, so they stay clean as they grow. Drag a node and the diagram switches to your arrangement, saved beside the file in `.schematlas/layout/` — the Mermaid source itself is untouched. **Tidy up** deletes that arrangement and returns to automatic layout.

draw.io files keep their own geometry: moving a node there moves it in the file.

## Agents, with review

Your coding agent reads the code with its own tools. To change documentation it uses Schematlas tools — `write_diagram`, `edit_diagram`, `write_doc`, `write_toc` — which validate the proposal and open a review in the app. Nothing is written before you allow it, whatever permission mode the agent runs in. See [Agents](/docs/agents).
