---
title: Getting started
description: Install Schematlas, open a repository and map it with your agent.
---

## Install

Download the installer for your platform from the [releases page](https://github.com/Jacqkues/schematlas/releases):

| Platform | Download |
| --- | --- |
| macOS | `.dmg` for Apple silicon or Intel |
| Windows | `.exe` or `.msi` installer, x64 |
| Linux | `.AppImage` or `.deb`, x64 |

The builds are not notarized or publisher-signed yet, so your system asks for confirmation the first time. On macOS, see [installing on macOS](https://github.com/Jacqkues/schematlas/blob/main/docs/macos-releases.md).

To build from source instead, follow the [README](https://github.com/Jacqkues/schematlas#build-from-source).

## Open a repository

1. On the projects page, choose **Add repository** and pick a folder. A project named after the folder opens.
2. The explorer lists the diagrams and docs in `.schematlas/toc.yaml`, and below them, under **In the repository**, every other diagram file it found: Mermaid, draw.io, DBML, and Mermaid blocks in Markdown.
3. Open any diagram. Files without saved positions are laid out automatically.

## Map it with your agent

Schematlas works with the coding agents you already have installed: Claude Code, Codex, Gemini CLI, OpenCode, or any agent that speaks ACP. It uses their existing login; there is no Schematlas account.

1. In the project home, type a question, or pick **Map the architecture**, **Explain a flow** or **Generate the wiki**.
2. Choose an agent and a model, then send.
3. The agent reads your code with its own tools and proposes diagrams and pages through Schematlas tools. Each proposal opens a review: allow it and the file is written.

> Agents never write to `.schematlas/` directly. Every diagram, page or TOC change goes through a validated proposal you approve.

## Draw by hand

- Double-click the canvas to add a node, and drag from a node's edge to connect it.
- Select a node and open **Details** to add a note, a kind, a color or code references.
- Drag nodes to arrange them. **Tidy up** returns the diagram to automatic layout.

Every edit is written back to the original file, in its own format.
