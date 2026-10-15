---
title: Privacy and storage
description: What Schematlas stores, where, and what never leaves your machine.
---

## In your repository

- Diagrams, wiki pages and the table of contents are files under `.schematlas/`, plus the diagram files you already had.
- Arrangements you make by dragging are saved in `.schematlas/layout/`.
- Team sync, when you turn it on, uses a `schematlas-sync` branch on your own remote.

## On your machine

- Projects, schema snapshots and preferences are stored locally and unencrypted. On macOS: `~/Library/Application Support/local.schema-atlas.desktop/`.
- Schema snapshots contain table, column and endpoint names, which can be sensitive on their own.

## Never stored

- **Credentials.** Database connection strings and API authentication headers stay in memory, are never written to disk and never logged. After a restart, reconnect a source before refreshing or querying it; its saved map stays browsable.
- **Conversations.** Agent transcripts stay with the agent. Schematlas keeps only the session id, to resume the conversation.
- **Telemetry.** The app has none.

## What your agent sees

Your coding agent may send prompts, code, schemas and the results of queries you approve to its model provider, according to its own configuration. Schematlas tools only run inside the project, and SQL or HTTP requests run only after you approve them.
