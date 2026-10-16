---
title: Code changes
description: When the code a node cites changes on your main branch, Schematlas flags it.
---

Each node's code references are compared with your reference branch — `main` by default, `origin/main` when it exists. If lines the node cites changed, the node shows **changed**, and so do its diagram and folders in the sidebar. A deleted file shows **broken**.

Open the node to see the commits and the diff, then:

- **Reviewed** — the node still holds. Its references move to where the code is now, and the commit is recorded (`commit=…`).
- **Ask agent to update** — your agent revises the node.

Only changes inside a cited range count. Schematlas only reads git: it never fetches or touches your working tree.
