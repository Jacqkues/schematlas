---
title: Introduction
description: Schematlas turns a repository into living documentation, with diagrams and docs that stay files in your code.
---

Schematlas is a desktop app for understanding a codebase. Open a repository, and you — or your local coding agent — map how it works: architecture diagrams, flows, a wiki, database schemas. Every map can point at the code it describes, and tells you when that code changes.

## Files, not a platform

Everything Schematlas shows is a file in your repository:

- **Diagrams** are Mermaid, draw.io and DBML files. The ones you already have work as they are.
- **Docs** are Markdown pages under `.schematlas/`.
- **Navigation** is one file, `.schematlas/toc.yaml`.

Schematlas adds what those formats cannot hold — links to code, notes, kinds, colors — as comments or properties that the original tools ignore. Your files stay valid, readable on GitHub, and reviewable in a pull request. Nothing is copied into a database or a cloud service.

## What you can do

- **Explore** a repository's diagrams and docs in one explorer, with clean automatic layout.
- **Ask your agent** — Claude Code, Codex, or any ACP agent — to map a flow or write the wiki. Every change it proposes waits for your approval.
- **Follow a node to its code**, and see when that code changes on your main branch.
- **Connect a database** to map its live schema, or import an OpenAPI definition.
- **Share presence and comments** with teammates through your own git remote, if you want.

## Where to go next

- [Getting started](/docs/getting-started) installs the app and maps a first repository.
- [How it works](/docs/how-it-works) explains the files Schematlas reads and writes.
- [Annotations](/docs/annotations) is the reference for the data Schematlas adds to each format.
