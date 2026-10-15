---
title: Code changes
description: When the code a node cites changes on your main branch, the node, its diagram and the sidebar say so.
---

Diagrams go stale quietly. Schematlas compares the code each node cites with your reference branch and flags the nodes whose code moved on, so you know which parts of the map to check.

## What you see

- A **changed** badge on the node, and a count beside the diagram in the sidebar and on its folders.
- A chip in the diagram header — *1 node changed · origin/main* — that opens the first flagged node.
- In the node's details: the reference that changed, where it is now, the commits that touched it, and the diff.

From there, choose **Reviewed** if the node still says what the code does, or **Ask agent to update** to have your agent revise it.

## What counts as a change

Only changes inside a cited line range count. If lines were added above the range, the reference simply moves with them.

| Status | Meaning |
| --- | --- |
| Changed | Lines the node cites changed on the reference branch |
| Broken | A cited file was deleted |
| Local | The change is only in your working copy (shown when **Local changes** is on) |

Renamed files are followed.

## Compared with what

Each node is compared from a **baseline** commit, the first of:

1. the node's own `commit=` annotation, set when it is reviewed;
2. the diagram's `commit=` annotation;
3. the last commit that touched the diagram file.

…to the **reference branch**: the branch chosen in the project settings, otherwise `main` or `master`. The remote version, `origin/main`, is preferred when it exists, so you see what your team merged — after your usual `git fetch`.

## Reviewed

**Reviewed** updates the node's references to where the code is now — shifted lines, renamed files — and records the commit it was checked against:

```text
%% @schematlas node api code=src/api.rs:14-82 commit=9b41d07
```

It is an ordinary edit of the diagram file, so it shows up in your next commit.

> Schematlas only reads git: it never fetches, checks out, or touches your index.
