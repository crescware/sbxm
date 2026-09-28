---
title: '`sbxm sync`'
description: Sync the host repository and the sandbox of a project added with --local, by Git's own rules.
---

```text
sbxm sync [<project-id>]
```

`sync` is for projects added with [`sbxm add --local`](../add/). The host repository plays the part GitHub plays for a GitHub project: the sandbox's branches and tags reach it the way `git push` would take them, and its branches and tags reach the sandbox's `origin` the way `git fetch --prune` would bring them. Git and the host repository's settings decide what moves. The one exception is a branch checked out on the host, which moves the way `git merge --ff-only` would move it in its worktree; see [The branch checked out on the host](#the-branch-checked-out-on-the-host). In an interactive terminal the project ID may be omitted and selected from the projects added with `--local`; when there is none, `sync` says so instead of asking. The sandbox must be running; a stopped sandbox is not started.

1. The sandbox's branches, tags, and each worktree's `HEAD` are saved under `refs/sbx/<sandbox>/` of the host repository. The host repository fetches them over `ssh <sandbox>.sbx`, the same connection that [`open`](../open/) uses, with `transfer.fsckObjects`. Only objects the host does not have yet are carried. The connection starts from the host; nothing in the sandbox can reach it. A branch rewritten or deleted in the sandbox keeps its previous tip under `refs/sbx/<sandbox>/archive/<time>/`, which is never removed automatically.
2. The host repository pushes what was saved into its own branches and tags: `refs/sbx/<sandbox>/heads/*` to `refs/heads/*` and `refs/sbx/<sandbox>/tags/*` to `refs/tags/*`. The host repository's own receive hooks run; its `pre-push` hook does not, since nothing leaves the repository. A branch checked out on the host is fast-forwarded in its worktree instead.
3. The host repository pushes its branches into the sandbox's `origin/*`, removing those deleted on the host, and its tags to the same names, over the same connection.

Because the host is brought up to date before it is sent back, `origin/<branch>` in the sandbox shows the host as it is after the sync.

## What Git refuses

Git moves a host branch only when the move is a fast-forward, and never moves a tag that already points elsewhere. A branch deleted in the sandbox stays on the host, as a remote branch stays on GitHub when you delete your own. A branch deleted on the host disappears from the sandbox's `origin`. Tags are never removed on either side.

Because deletions do not travel, a tag you delete on the host comes back at the next sync while the sandbox still has it, as it would after `git push --tags` from a clone that has it; delete it inside the sandbox as well. The same goes for a branch deleted on the host while the sandbox has its own branch of that name.

| Result | Meaning |
| --- | --- |
| `created` | The branch or tag did not exist in the host repository and was created |
| `updated` | The host branch moved forward to the sandbox's tip; if it is checked out on the host, the files of its worktree moved with it |
| `behind` | The sandbox branch is only behind the host branch; nothing is lost |
| `diverged` | The host branch and the sandbox branch each have commits the other lacks |
| `local-changes` | The branch is checked out on the host, and uncommitted changes there overlap the sandbox's commits, so it was not moved; the files are named |
| `checked-out` | The branch is checked out on the host, but not in exactly one worktree that points at it, such as one in the middle of a rebase, so it was not moved |
| `exists` | A tag of this name already points elsewhere in the host repository |
| `refused` | Git refused the update for another reason, such as a receive hook; the reason is shown |

In the sandbox's `origin`, a ref is `created`, `updated`, or `removed`, or `refused` when the sandbox already has a tag of that name pointing elsewhere; the sandbox's own tag is left.

Whatever was left as it was keeps its commits under `refs/sbx/<sandbox>/`. To bring a `diverged` branch in, merge or rebase onto `origin/<branch>` inside the sandbox, then sync again, the same as you would before pushing to GitHub again.

When anything was left as it was, that is, any result other than `created`, `updated`, or `behind` in the host repository, or `refused` in the sandbox's `origin`, `sync` does not say it synced. It shows the whole result under a warning and exits with status `1`, as `git push` and `git fetch` do. A sandbox branch that is only `behind` does not count: the host already has its commits.

A project added from GitHub is refused: its sandbox fetches from and pushes to GitHub itself.

## The branch checked out on the host

The user who runs `sync` is at the host and expects it to move, so a branch checked out on the host is moved when the move is unambiguous: when `git merge --ff-only` run by hand in the worktree that has it checked out would go through without asking anything and without a conflict. `sync` runs exactly that there. Uncommitted changes to other files stay as they are; changes, staged or not, to a file the sandbox's commits also change, or an untracked file where they add one, stop it as `local-changes` and are named. Commit or stash them on the host, then sync again. Any other reason Git gives, such as a merge in progress, is shown as `refused` with Git's answer.

`receive.denyCurrentBranch` in the host repository does not apply: whatever it is set to, the branch and the files of its worktree move together, or neither moves. The host repository's `pre-receive` hook still sees the branch as part of the push and can refuse it. Its `update` and `post-receive` hooks do not run for the branch; its `post-merge` hook does, as it would for the same merge by hand.

A branch that no single worktree points at, such as one in the middle of a rebase or bisect, or one checked out in a worktree whose directory is gone, is left as `checked-out`.

## The sandbox side

`sync` never touches the sandbox's worktrees, branches, or `HEAD`; only its `origin/*` and tags change. An agent may be working in the sandbox without the user who runs `sync` being there. After the sync, `sync` names the sandbox branches that lack commits the host has. A branch that is only `behind` takes them in with a fast-forward onto `origin/<branch>` inside the sandbox. A `diverged` branch needs a merge or rebase onto `origin/<branch>` and another sync to bring the result to the host.
