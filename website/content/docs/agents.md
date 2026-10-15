---
title: Agents
description: Use the coding agent you already have. Schematlas gives it documentation tools and reviews every change.
---

## Supported agents

| Agent | Connection |
| --- | --- |
| Claude Code | Its native stream-json protocol |
| Codex | `codex app-server` |
| Gemini CLI, OpenCode | Their built-in ACP mode |
| Any ACP agent | **Custom agent…**, with its command and arguments |

Agents run in the project folder with their own login, model and settings. Schematlas finds installed agents without launching them.

## Modes

Choose the agent's own permission mode in the composer — for Claude Code *Ask first*, *Accept edits*, *Auto*, *Plan* and the others; for Codex *Read only*, *Auto* or *Full access*. Modes govern the agent's own tools, like editing source files or running commands. Schematlas tools work in every mode, and never skip their own review.

## What the agent knows

The file you have open is sent with your message as one line of context:

```text
Open in Schematlas: diagram `.schematlas/diagrams/checkout.mmd`, selected node `api` (“Orders API”).
```

It shows as a chip in the composer; dismiss it to send the message without it.

## Tools

Schematlas gives the agent a project-scoped set of tools. Tools that change files or touch a database wait for your approval.

| Tool | Does | Review |
| --- | --- | --- |
| `repo_outline` | Summarize the repository: languages, entry points, folders | |
| `list_diagrams` | List diagrams and docs | |
| `get_diagram`, `read_diagram_source` | Read a diagram | |
| `write_diagram` | Create or replace a Mermaid diagram | Yes |
| `edit_diagram` | Add or remove nodes and edges, relabel, annotate | Yes |
| `read_doc`, `write_doc` | Read or write a wiki page | Write |
| `read_toc`, `write_toc` | Read or reorganize the table of contents | Write |
| `check_refs` | Find code references that no longer resolve | |
| `list_sources`, `search_schema`, `describe_table`, `get_schema`, `find_join_path` | Explore connected schemas | |
| `table_stats`, `sample_rows`, `explain_sql`, `query_sql` | Query a database | Yes |
| `request_http` | Call a documented API operation | Yes |
| `show_in_app` | Open a diagram, doc or data source in your window, and select a node | |
| `move_diagram_nodes` | Arrange nodes of a diagram, like a drag | draw.io only |
| `get_canvas`, `move_nodes`, `create_group`, `remove_group` | Arrange a schema canvas | |

Proposals are validated before you see them: a diagram must parse, a page's citations must resolve. The review shows the change — nodes added, changed and removed, or a Markdown diff — and the file is written only if you allow it and it has not changed in the meantime.

## Generating the wiki

**Generate the wiki** asks the agent to survey the repository with `repo_outline`, propose a structure in `toc.yaml` and wait for your approval, then write the pages and their diagrams with code references.

## Conversations

Conversations belong to the agent. Schematlas keeps only their ids, lists them in the project home, and reopens them with the agent's own resume — Claude's `--resume`, Codex's `thread/resume`, ACP's `session/load`. The transcript is never copied.
