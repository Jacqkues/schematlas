---
title: Team sync and privacy
description: What is shared, what is stored, and what never leaves your machine.
---

## Team sync

Off until you turn it on in the **Team** panel. It shares presence, comments and change notes through a `schematlas-sync` branch on your own git remote — no server, no account. Your working tree and branches are never touched, and your email is never written: authors are identified by a key derived from it.

## What is stored

- **In your repository:** diagrams, pages, the TOC and arrangements, under `.schematlas/`.
- **On your machine:** projects and schema snapshots, unencrypted.
- **Never:** database credentials and API headers, which stay in memory; agent transcripts, which stay with the agent; telemetry, which the app has none of.

Your agent may send prompts, code and approved query results to its model provider, according to its own settings.
