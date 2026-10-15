---
title: DBML
description: Database schemas as text in your repository, drawn by the same canvas as a live connection, with notes in comment lines.
---

## Drawn like a database

A `.dbml` file opens in the schema canvas, the same one used for a connected database: tables with their columns and keys, relationships with cardinality, and `TableGroup` blocks as named groups. You can arrange tables and annotate them; the structure itself is edited in the DBML text.

Arrangements are saved in `.schematlas/layout/`, never in the DBML file.

## Annotation lines

DBML comments start with `//`, so Schematlas annotations use the same grammar as Mermaid with that prefix:

```text
// @schematlas diagram title="Commerce" note="Production schema, read-only replica"
// @schematlas node public.orders code=src/orders/model.rs:12-60 note="One row per checkout"
// @schematlas node public.customers kind=entity tags=pii
```

- Tables are always named with their schema: `public.orders`. Tables without a schema are in `public`.
- Relationships cannot be annotated in DBML.
- New annotation lines are added at the end of the file.

## Saving a live database as DBML

A connected database can be saved into the repository, by default as `.schematlas/schema/<name>.dbml`. It becomes a plain file your team can review, and an agent can read without database access.

- **The first save** writes canonical DBML: `Project`, enums, tables, refs, table groups, then annotations.
- **Later saves merge** the live schema into the existing file, element by element:
  - unchanged tables and columns keep their exact text;
  - new tables and columns are added, removed ones deleted;
  - your notes, comments, aliases, groups, `Project` block and `// @schematlas` lines are kept.

The merged file must describe exactly the live schema when read back; if it would not, the save is refused instead of writing something wrong. Only structure is compared — tables, columns, indexes, refs and enums — never notes or groups.

## Example

```dbml
Project shop {
  database_type: 'PostgreSQL'
}

Table public.orders {
  id integer [pk]
  customer_id integer [ref: > public.customers.id]
  total numeric
  Note: 'Written by the checkout service'
}

Table public.customers {
  id integer [pk]
  email text [unique]
}

TableGroup ordering {
  public.orders
}

// @schematlas node public.orders code=src/orders/model.rs:12-60
```
