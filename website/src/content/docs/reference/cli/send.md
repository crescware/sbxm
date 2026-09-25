---
title: '`sbxm send`'
description: Send the branches and tags of the host repository to the sandbox of a project added with --local.
---

```text
sbxm send [<project-id>]
```

`send` is for projects added with [`sbxm add --local`](../add/). In an interactive terminal the project ID may be omitted and selected. The sandbox must be running; a stopped sandbox is not started.

1. The host repository's branches and tags are written into one `git bundle` in the project's `.sbxm/bundles`, and removed again once it has been sent.
2. The bundle is streamed into the sandbox through the standard input of `sbx exec`. Inside the sandbox it is received next to where it belongs, its digest is checked, and it replaces `<repository>/.git/sbxm/origin.bundle` with a rename. A bundle that does not arrive whole replaces nothing.
3. `git fetch --prune origin` runs in the sandbox. A branch deleted on the host disappears from the sandbox's origin too.

Before sending, `send` confirms that the sandbox holds this project's repository; a sandbox whose build did not get that far receives nothing. The sandbox's worktrees and branches are never touched: merge or rebase onto `origin/<branch>` inside the sandbox to take the changes in.

The result lists each `refs/remotes/origin/*` and tag that was `created`, `updated`, or `removed`.

A project added from GitHub is refused: its sandbox fetches from GitHub itself.
