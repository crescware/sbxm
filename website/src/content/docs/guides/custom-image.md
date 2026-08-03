---
title: Customize the sandbox image
description: Edit a project Dockerfile and safely apply the new sandbox image.
---

`sbxm add` creates a Dockerfile in the project’s host-side directory. Edit that file to add tools or system dependencies.

## Rebuild

```sh
sbxm rebuild <project-id>
```

Rebuild applies the edited Dockerfile by recreating the sandbox. It also rebuilds the repository setup and managed worktrees.

## Before rebuilding

The normal rebuild protects work by checking for:

- dirty files
- unpushed commits
- an in-progress Git operation
- unmanaged worktrees
- a Dockerfile or artifact that belongs to another project

Commit and push what you want to keep, remove what you do not need, and inspect unmanaged worktrees yourself. sbxm does not delete unrelated work to make a rebuild fit.

## Add worktrees without rebuilding

If only the number of managed worktrees changes, use:

```sh
sbxm apply <project-id> --worktrees 4
```

Use [managed worktrees](../worktrees/) for the count and attached/detached behavior.
