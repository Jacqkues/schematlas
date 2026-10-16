---
title: How it works
description: Schematlas reads the files in your repository and writes every change back to them.
---

## Your files

| What | Where |
| --- | --- |
| Diagrams you already have | Anywhere: `.mmd`, `.mermaid`, `.drawio`, `.dbml`, and Mermaid blocks in Markdown |
| Diagrams made in Schematlas | `.schematlas/diagrams/`, as Mermaid |
| Wiki pages | `.schematlas/wiki/`, as Markdown |
| Saved database schemas | `.schematlas/schema/`, as DBML |
| Navigation | `.schematlas/toc.yaml` |
| Arrangements you made by dragging | `.schematlas/layout/` |

Schematlas edits a file by changing only what you changed, so your formatting and comments stay. If a file changed on disk meanwhile — an editor, a `git pull`, your agent — the save is refused and the new version is shown.

## Notes inside the files

To link a node to its code, or give it a note or a color, Schematlas writes the information where the original tool ignores it: a comment line in Mermaid and DBML, a custom property in draw.io. Your files still open everywhere else. See [Diagram formats](/docs/formats).

## Layout

Diagrams are laid out automatically, with the algorithm Mermaid uses. Drag a node and your arrangement is saved beside the file; **Tidy up** returns to automatic layout. draw.io keeps its own positions in the file. Sequence diagrams are always drawn in time order.

## The table of contents

`.schematlas/toc.yaml` orders the explorer. Paths are relative to the repository root, and moving an entry never moves a file:

```yaml
version: 1
items:
  - title: "Wiki"
    children:
      - title: "Overview"
        file: ".schematlas/wiki/overview.md"
  - title: "Checkout"
    file: "docs/checkout.mmd"
```

Diagram files the TOC does not list appear under **In the repository**, where you can add them.
