---
title: Team sync
description: Presence, comments and change notes shared through a branch of your own git remote. No server, no account.
---

Team sync is off until you turn it on for a repository in the **Team** panel. It shows the remote, its address, the branch and the name teammates will see, and pushes nothing before you confirm.

## What is shared

- **Presence**: who is looking at which diagram.
- **Comments** on nodes, and their resolution.
- **Chat** and notes when a diagram changes.

## How it works

Everything travels on a `schematlas-sync` branch of the repository's remote:

```text
meta.json
events/<author-key>/<YYYY-MM-DD>.jsonl
presence/<author-key>.json
```

- Each person appends to their own files, so pushes never conflict; a lost race is rebuilt and retried.
- The app polls about every 15 seconds while its window is focused.
- It uses git plumbing only: your HEAD, index and working tree are never touched, and your usual credentials authenticate.
- Commits are titled `schematlas sync [skip ci]`. For CI systems that ignore that marker, exclude the branch.
- Events older than 14 days are dropped.

## Privacy

Everyone who can read the repository can read the branch. Your email is never written: authors are identified by a key derived from it. Remote addresses are stored without credentials, and if a remote starts pointing somewhere else, sync pauses until you confirm again.
