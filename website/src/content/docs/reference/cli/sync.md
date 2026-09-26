---
title: '`sbxm sync`'
description: Sync the host repository and the sandbox of a project added with --local, by Git's own rules.
---

```text
sbxm sync [<project-id>]
```

`sync` is for projects added with [`sbxm add --local`](../add/). The host repository plays the part GitHub plays for a GitHub project: the sandbox's branches and tags reach it the way `git push` would take them, and its branches and tags reach the sandbox's `origin` the way `git fetch --prune` would bring them. Git and the host repository's settings decide what moves; sbxm adds no rule of its own. In an interactive terminal the project ID may be omitted and selected from the projects added with `--local`; when there is none, `sync` says so instead of asking. The sandbox must be running; a stopped sandbox is not started.

1. The sandbox's branches, tags, and each worktree's `HEAD` are saved under `refs/sbx/<sandbox>/` of the host repository. The host repository fetches them over `ssh <sandbox>.sbx`, the same connection that [`open`](../open/) uses, with `transfer.fsckObjects`. Only objects the host does not have yet are carried. The connection starts from the host; nothing in the sandbox can reach it. A branch rewritten or deleted in the sandbox keeps its previous tip under `refs/sbx/<sandbox>/archive/<time>/`, which is never removed automatically.
2. The host repository pushes what was saved into its own branches and tags: `refs/sbx/<sandbox>/heads/*` to `refs/heads/*` and `refs/sbx/<sandbox>/tags/*` to `refs/tags/*`. The host repository's own receive hooks run; its `pre-push` hook does not, since nothing leaves the repository.
3. The host repository pushes its branches into the sandbox's `origin/*`, removing those deleted on the host, and its tags to the same names, over the same connection.

Because the host is brought up to date before it is sent back, `origin/<branch>` in the sandbox shows the host as it is after the sync.

## What Git refuses

Git moves a host branch only when the move is a fast-forward, never moves the branch checked out in the host repository, and never moves a tag that already points elsewhere. A branch deleted in the sandbox stays on the host, as a remote branch stays on GitHub when you delete your own. A branch deleted on the host disappears from the sandbox's `origin`. Tags are never removed on either side.

Because deletions do not travel, a tag you delete on the host comes back at the next sync while the sandbox still has it, as it would after `git push --tags` from a clone that has it; delete it inside the sandbox as well. The same goes for a branch deleted on the host while the sandbox has its own branch of that name.

| Result | Meaning |
| --- | --- |
| `created` | The branch or tag did not exist in the host repository and was created |
| `updated` | The host branch moved forward to the sandbox's tip |
| `behind` | The sandbox branch is only behind the host branch; nothing is lost |
| `diverged` | The host branch and the sandbox branch each have commits the other lacks |
| `checked-out` | The branch is checked out in the host repository, so Git did not move it |
| `exists` | A tag of this name already points elsewhere in the host repository |
| `refused` | Git refused the update for another reason, such as a receive hook; the reason is shown |

In the sandbox's `origin`, a ref is `created`, `updated`, or `removed`, or `refused` when the sandbox already has a tag of that name pointing elsewhere; the sandbox's own tag is left.

A branch checked out on the host moves along with its files when the host repository sets `receive.denyCurrentBranch` to `updateInstead` and has no uncommitted changes, as for any push into it.

Whatever Git refused stays as it was, and its commits are kept under `refs/sbx/<sandbox>/`. To bring such a branch in, merge or rebase onto `origin/<branch>` inside the sandbox, then sync again, the same as you would before pushing to GitHub again. The sandbox's worktrees and branches are never touched by `sync`.

When Git left any ref as it was, that is, any result other than `created`, `updated`, or `behind` in the host repository, or `refused` in the sandbox's `origin`, `sync` still shows the whole result and then exits with status `1`, as `git push` and `git fetch` do. A sandbox branch that is only `behind` does not count: the host already has its commits.

A project added from GitHub is refused: its sandbox fetches from and pushes to GitHub itself.
