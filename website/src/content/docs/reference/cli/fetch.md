---
title: '`sbxm fetch`'
description: Save the commits of a project sandbox into its host repository without touching your branches.
---

```text
sbxm fetch [<project-id>]
```

In an interactive terminal the project ID may be omitted and selected. The sandbox must be running; a stopped sandbox is not started.

`fetch` never copies the sandbox's `.git` directory: hooks and configuration would come with it, and a copy taken during a Git operation can be inconsistent. Instead:

1. Inside the sandbox, the branches, tags, and each worktree's `HEAD` are written as one `git bundle` to standard output of `sbx exec`. A worktree's `HEAD` may point at a commit on no branch, so it is included under a temporary ref that is removed afterwards.
2. The bundle is received into the project's `.sbxm/bundles/<time>.bundle`, never over an existing file, with a size limit. Only the newest few bundles are kept; the imported commits live in the repository refs.
3. The bundle is checked with `git bundle verify`, then fetched with `transfer.fsckObjects` into `refs/sbx/<sandbox>/` of the host repository, with `--no-tags`. Host branches and tags are never touched.

| Sandbox ref | Host ref |
| --- | --- |
| `refs/heads/<name>` | `refs/sbx/<sandbox>/heads/<name>` |
| `refs/tags/<name>` | `refs/sbx/<sandbox>/tags/<name>` |
| a worktree's `HEAD` | `refs/sbx/<sandbox>/worktrees/<worktree>` |

When a ref was rewritten so that its new tip does not contain the previous one, or was deleted in the sandbox, the previous tip is first kept under `refs/sbx/<sandbox>/archive/<time>/` and is never removed automatically. Every commit that was ever fetched therefore stays reachable. No sandbox ref is mapped into `archive/`.

The result lists each ref that was `created`, `updated`, `replaced`, or `deleted`, with where a previous tip was kept.

Every tip under `refs/sbx/<sandbox>/`, including the kept ones, outlives the sandbox. [`rebuild`](../rebuild/) and [`destroy`](../destroy/) therefore count a commit reachable from them as published, the same as a commit reachable from the origin, and offer to run the same save when unpublished commits on branches, tags, or worktree `HEAD`s are the only thing stopping them. A stash or notes commit is not carried by the bundle, so saving does not keep it.
