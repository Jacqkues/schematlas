---
title: Wiki pages
description: Markdown pages that embed live diagrams and cite the code they explain.
---

Wiki pages are Markdown files, usually under `.schematlas/wiki/`, ordered by the [table of contents](/docs/table-of-contents). They read like any Markdown on GitHub, with three additions Schematlas understands.

## Embedded diagrams

A line that is only a link to `schematlas:diagram/<id>` is drawn as the live diagram, laid out like the editor, with a button to open it:

```markdown
## How checkout works

[Checkout flow](schematlas:diagram/.schematlas/diagrams/checkout.mmd)
```

A fenced `mermaid` code block in the page is drawn inline too.

## Code citations

Cite code as `path:line`, `path:start-end` or `path#Lstart-Lend`, in backticks or as a link target:

```markdown
The basket is validated in `src/api/checkout.rs:18-64`,
then the [order is saved](src/orders/repo.rs:40).
```

Citations are listed in the page's **Relevant sources** panel.

## Front matter

```yaml
---
title: Payments
sources:
  - src/payments/
  - src/api/checkout.rs
commit: 3f2c9e1
---
```

`sources` adds files to the Relevant sources panel; `commit` records what the page was written against.

## Checks before a page is written

When an agent proposes a page, Schematlas checks it before showing the review:

- every cited file exists, and cited lines are within the file;
- every embedded diagram id exists;
- every Mermaid block parses.

A page that fails is sent back to the agent with the reasons, instead of reaching you.
