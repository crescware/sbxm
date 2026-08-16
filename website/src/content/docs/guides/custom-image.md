---
title: Customize the sandbox image
description: Edit a project Dockerfile and safely apply the new sandbox image.
---

`sbxm add` creates a Dockerfile in the project’s host-side directory. Edit that file to add tools or system dependencies.

## Rebuild

```sh
sbxm rebuild <project-id>
```

Rebuild applies the Dockerfile by recreating the sandbox, whether or not the
file changed. The old writable layer is lost. It also rebuilds the repository
setup and managed worktrees.

## Before rebuilding

The normal rebuild protects work by checking for:

- dirty files
- unpublished commits
- an in-progress Git operation
- repository-level refs such as local branches, tags, notes, stash entries,
  extra remotes, and reflog-only commits
- unmanaged worktrees
- a Dockerfile or artifact that belongs to another project

Commit and push what you want to keep, remove what you do not need, and
inspect unmanaged worktrees yourself. A clean worktree does not prove that the
repository has no Layer A blocker. sbxm does not delete unrelated work to make
a rebuild fit.

## Add worktrees without rebuilding

If only the number of managed worktrees changes, use:

```sh
sbxm apply <project-id> --worktrees 4
```

Use [managed worktrees](../worktrees/) for the count and attached/detached behavior.
