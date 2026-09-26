---
title: '`sbxm fetch`'
description: Save the commits of a project sandbox into its host repository without touching your branches.
---

```text
sbxm fetch [<project-id>]
```

In an interactive terminal the project ID may be omitted and selected. The sandbox must be running; a stopped sandbox is not started.

`fetch` never copies the sandbox's `.git` directory: hooks and configuration would come with it, and a copy taken during a Git operation can be inconsistent. Instead:

1. Inside the sandbox, each worktree's `HEAD` is placed under a temporary ref, because it may point at a commit on no branch. A temporary ref left by a save that stopped halfway is removed first.
2. The host repository fetches the sandbox's branches, tags, and those temporary refs over `ssh <sandbox>.sbx`, the same connection that [`open`](../open/) uses, with `transfer.fsckObjects` and `--no-tags`, into `refs/sbx/<sandbox>/`. Only objects the host does not have yet are carried. The connection starts from the host; nothing in the sandbox can reach it. SSH never prompts and never forwards the host's SSH agent. Host branches and tags are never touched.
3. The temporary refs in the sandbox are removed, whether the fetch succeeded or not.

| Sandbox ref | Host ref |
| --- | --- |
| `refs/heads/<name>` | `refs/sbx/<sandbox>/heads/<name>` |
| `refs/tags/<name>` | `refs/sbx/<sandbox>/tags/<name>` |
| a worktree's `HEAD` | `refs/sbx/<sandbox>/worktrees/<worktree>` |

When a ref was rewritten so that its new tip does not contain the previous one, or was deleted in the sandbox, the previous tip is first kept under `refs/sbx/<sandbox>/archive/<time>/` and is never removed automatically. Every commit that was ever fetched therefore stays reachable. No sandbox ref is mapped into `archive/`.

The result lists each ref that was `created`, `updated`, `replaced`, or `deleted`, with where a previous tip was kept.

Every tip under `refs/sbx/<sandbox>/`, including the kept ones, outlives the sandbox. [`rebuild`](../rebuild/) and [`destroy`](../destroy/) therefore count a commit reachable from them as published, the same as a commit reachable from the origin, and offer to run the same save when unpublished commits on branches, tags, or worktree `HEAD`s are the only thing stopping them. A stash or notes commit is not carried, so saving does not keep it.
