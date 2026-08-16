---
title: Managed worktrees
description: Use attached or detached Git worktrees for independent sbxm tasks.
---

Every project starts with one managed worktree. The supported count is **1–32**.

## Attached mode

The default registration keeps the first worktree on the repository’s default branch:

```sh
sbxm add git@github.com:<owner>/<repository>.git
```

For an attached project, additional worktrees created later are detached. The first worktree remains the tracking worktree.

## Detached mode

Choose an explicit starting branch when you want several independent worktrees:

```sh
sbxm add git@github.com:<owner>/<repository>.git \
  --detach main --worktrees 3
```

Each managed worktree starts detached from the selected branch. This is useful when multiple agents or tasks need isolated directories.

## Add worktrees later

The count can only increase:

```sh
sbxm apply <project-id> --worktrees 4
```

`apply` refuses a lower number. Removing a worktree can delete whatever is checked out there, so reduction is not a side effect of re-running a command. Destroy the project only when that deletion is intentional.

## Protection

Before a rebuild or destroy, sbxm checks dirty files, unpublished commits,
repository-level refs, unmanaged worktrees, and active sessions. Save work and
resolve the reported condition before retrying. A clean worktree does not by
itself make a rebuild safe.
