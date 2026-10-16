---
title: Introduction
description: Diagrams and docs for your repository, written with your agent and kept as files in your code.
---

Schematlas is a desktop app for understanding a codebase. You — or your coding agent — map how it works: architecture diagrams, sequence diagrams, a wiki, database schemas. Every map can point at the code it describes, and tells you when that code changes.

Everything is a file in your repository. Your Mermaid, draw.io and DBML files work as they are, docs are Markdown under `.schematlas/`, and nothing is copied into a cloud service.

## Install

Download the installer for macOS, Windows or Linux from the [releases page](https://github.com/Jacqkues/schematlas/releases). The builds are not signed yet, so your system asks for confirmation the first time ([help for macOS](https://github.com/Jacqkues/schematlas/blob/main/docs/macos-releases.md)). To build from source, see the [README](https://github.com/Jacqkues/schematlas#build-from-source).

## First steps

1. **Add a repository** on the projects page. The explorer lists its diagrams and docs.
2. **Ask your agent** from the project home — Claude Code, Codex or any ACP agent you have installed. Try *Map the architecture* or *Generate the wiki*.
3. **Review** what it proposes. Nothing is written until you allow it.
4. **Open a diagram**, select a node, and follow it to the code.

You can also draw by hand: double-click the canvas to add a node, drag from its edge to connect it.
