---
title: Agents
description: Use the coding agent you already have. Schematlas gives it tools and reviews its changes.
---

Schematlas works with **Claude Code**, **Codex**, **Gemini CLI**, **OpenCode** and any **ACP** agent, using their own login and models. Pick the agent, the model and its permission mode in the composer.

## What the agent can do

- Read the repository with its own tools, and an outline from `repo_outline`.
- Read and write diagrams (`write_diagram`, `edit_diagram`), wiki pages (`write_doc`) and the table of contents (`write_toc`).
- Show you things: `show_in_app` opens a diagram, page or database in your window and selects a node; `move_diagram_nodes` arranges a diagram.
- Explore connected databases, and run queries or API calls you approve.

## Review

Every change to a diagram, page or the TOC opens a review with the diff. SQL and HTTP requests wait for your approval of the exact statement. This holds in every permission mode — modes only govern the agent's own tools, like editing source files.

The file you have open is sent with your message as context. Conversations stay with the agent; Schematlas lists them on the project home and reopens them.
