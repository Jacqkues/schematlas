---
title: Table of contents
description: One YAML file orders the wiki, diagrams and data sources in the explorer.
---

`.schematlas/toc.yaml` is the source of truth for navigation. Its order is the explorer's order; nothing is sorted alphabetically. Moving an entry in the TOC never moves the file it points to.

## Example

```yaml
version: 1
items:
  - title: "Wiki"
    children:
      - title: "Overview"
        file: ".schematlas/wiki/overview.md"
      - title: "Diagrams"
        children:
          - title: "Checkout flow"
            file: ".schematlas/diagrams/checkout.mmd"
            type: "diagram"
  - title: "Infrastructure"
    file: "infra/network.drawio"
    page: 1
  - title: "README flow"
    file: "README.md"
    block: 0
  - title: "Production database"
    source: "Production"
```

## Entries

Each entry has a `title` (up to 120 characters) and exactly one of:

| Key | Points at |
| --- | --- |
| `file` | A doc or diagram, by repository-relative path |
| `children` | A folder of entries |
| `source` | A connected database or imported API, by name |

Optional keys:

| Key | Meaning |
| --- | --- |
| `type` | `doc`, `diagram`, `database` or `openapi` |
| `block` | For Markdown files: which Mermaid block, from 0 |
| `page` | For draw.io files: which page, from 0 |

Unknown keys are refused, so a typo is reported rather than ignored. A TOC holds up to 2,000 entries, nested up to 8 levels.

## Paths

Paths are relative to the repository root, so the TOC can list any file — `.schematlas/wiki/overview.md` as well as `docs/architecture.mmd`. A path cannot be absolute, contain `..` or `.git`, or point at `toc.yaml` itself or `.schematlas/layout/`.

## Files the TOC does not list

Diagram files found elsewhere in the repository appear under **In the repository**, below the TOC. Add one, or a whole folder, to the TOC from there.

> Schematlas rewrites `toc.yaml` when you reorganize the explorer, so comments in it are not kept.
