---
title: Annotations
description: The keys Schematlas adds to diagrams, and how each format stores them.
---

An annotation is a small set of keys attached to a diagram, a node or an edge. The keys are the same in every format; only where they are stored differs.

## Keys

| Key | Value | Example |
| --- | --- | --- |
| `title` | Text | `title="Checkout"` |
| `kind` | A word: `service`, `store`, `queue`, `actor`… | `kind=service` |
| `code` | A code reference; repeat for several | `code=src/api.rs:12-80` |
| `note` | Markdown text | `note="Validates carts"` |
| `tags` | Comma-separated words | `tags=http,core` |
| `color` | Accent color of the card | `color=#7fa98d` |
| `commit` | Commit the node was last reviewed against | `commit=3f2c9e1` |

Any other key is kept as it is, so a file written by a newer version of Schematlas survives an older one. Keys are written in a stable order — `title`, `kind`, `code`, `note`, `tags`, then the others alphabetically — so diffs stay small.

## Code references

| Form | Means |
| --- | --- |
| `src/api.rs` | The whole file |
| `src/api.rs:12` | One line |
| `src/api.rs:12-80` | A range of lines |
| `src/api.rs#L12-L80` | The same range, Markdown-link style |

Paths are relative to the repository root. Absolute paths, `..`, backslashes and drive letters are refused. References are always written back as `path:start-end`.

## Where each format stores them

| Format | Storage | Example |
| --- | --- | --- |
| Mermaid | `%%` comment lines | `%% @schematlas node api code=src/api.rs:12-80` |
| DBML | `//` comment lines | `// @schematlas node public.orders note="Orders"` |
| draw.io | Cell properties | `schematlas-code="src/api.rs:12-80"` |

Details: [Mermaid](/docs/mermaid), [DBML](/docs/dbml), [draw.io](/docs/drawio).

## Line syntax

Mermaid and DBML share one grammar:

```text
@schematlas diagram ...
@schematlas node <id> ...
@schematlas edge <from> <to> [n] ...
```

Values are single words or double-quoted strings with `\"`, `\\` and `\n` escapes. Lists repeat the key (`code=`), except `tags`, which is comma-separated. A key without a value is kept as empty.
