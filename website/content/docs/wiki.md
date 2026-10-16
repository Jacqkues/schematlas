---
title: Wiki pages
description: Markdown pages that embed live diagrams and cite the code they explain.
---

Wiki pages are Markdown, usually under `.schematlas/wiki/`, and read like any Markdown on GitHub.

- **Embed a diagram** with a line that is only a link: `[Checkout](schematlas:diagram/docs/checkout.mmd)`. A fenced `mermaid` block is drawn too.
- **Cite code** as `path:line` or `path:start-end`, in backticks. Citations appear in the page's **Relevant sources**.
- **Front matter** can list `sources:` and the `commit:` the page was written against.

When your agent writes a page, Schematlas checks it first: cited files and lines exist, embedded diagrams exist, Mermaid parses. A page that fails goes back to the agent instead of reaching you.
