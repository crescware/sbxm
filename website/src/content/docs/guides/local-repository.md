---
title: Develop without GitHub
description: Use a Git repository on this host as the origin of a disposable sandbox.
---

A repository that already lives on this host can be a project. sbxm does not clone it. The repository itself acts as the origin, and the sandbox is a disposable place to work in that you can recreate from the host. Pass the repository's Git directory, not its working tree.

```sh
cd ~/Projects
sbxm add --local ~/code/<repository>/.git
sbxm open local/<repository>
```

## Registering

The path must be a Git directory: the `.git` of a working tree, a bare repository, or the `.git` file of a worktree created with `git worktree add` or of a submodule. A working-tree directory is refused, because a directory can hold more than one repository and does not say which one it means; Git does not search upward from the path either. A symlink or a `.git` file is resolved, and the real path of the repository is recorded. For a worktree, that is the repository its worktrees share, so every worktree of one repository adds the same project.

Like any project, the project directory is created in the directory you run `add` from, so run it from outside the repository; `add` refuses to create it inside any working tree of the repository or inside its Git directory. The project ID is `local/<name>`, where the name is the directory name `git clone` would use — `<repository>` for `~/code/<repository>/.git`, `app` for `app.git` — unless you pass `--name <name>`. When that name cannot name a project, `add` asks for `--name`.

The sandbox starts from the branch the Git directory you passed is on; for a worktree's `.git`, that is the branch of that worktree. When it is detached, pass the starting branch with `--detach`. No GitHub token is involved, so there is nothing to register before `open`.

## Syncing with the host

Run [`sbxm sync`](../../reference/cli/sync/) to sync the host repository and the sandbox. The host repository plays the part GitHub plays for a GitHub project: the sandbox's branches and tags reach it the way `git push` would take them, and its branches and tags reach the sandbox's `origin` the way `git fetch --prune` would bring them. Git refuses what it would refuse in a push, such as a branch that is not a fast-forward, the branch checked out on the host, or a tag that already points elsewhere, and the refused commits stay under `refs/sbx/<sandbox>/`. Merge or rebase onto `origin/<branch>` inside the sandbox and sync again, as you would before pushing to GitHub again.

## Getting the host's history into the sandbox

When `open` builds the sandbox, sbxm writes the host repository's branches and tags into one `git bundle`. It streams the bundle into the sandbox through the standard input of `sbx exec` and places it at `<repository>/.git/sbxm/origin.bundle` after checking its digest. The sandbox's `origin` points at that file, so the managed worktrees are created from `origin/<branch>` as they are for a GitHub project. Nothing in the sandbox can reach the host repository.

[`sbxm apply --worktrees`](../../reference/cli/apply/) sends the host repository again before it adds worktrees, as `sbxm send` does, so the new worktrees start from the host's current branches.

## Bringing work back

Run [`sbxm fetch`](../../reference/cli/fetch/) to save the sandbox's commits into `refs/sbx/<sandbox>/` of the host repository. Your branches are never touched; merge what you want yourself, for example:

```sh
git merge refs/sbx/<sandbox>/heads/main
```

## Sending what the host gained

Run [`sbxm send`](../../reference/cli/send/) to send the host repository's current branches and tags. sbxm sends a fresh bundle and runs `git fetch --prune origin` inside the sandbox, so `origin/<branch>` there matches the host, and a branch deleted on the host disappears from the sandbox's origin too. The sandbox's worktrees and branches are left alone. Merge or rebase onto `origin/<branch>` inside the sandbox when you want the changes.

## Automatic fetch

Only work committed since the last fetch can be lost, so sbxm narrows that window by fetching on its own:

- before [`stop`](../../reference/cli/stop/) stops a running sandbox,
- before [`rebuild`](../../reference/cli/rebuild/) and [`destroy`](../../reference/cli/destroy/) check what would be lost, including `destroy --force`,
- every 10 minutes while an [`open`](../../reference/cli/open/) session is connected, silently, since the terminal belongs to SSH,
- after the session closes, with the project lock taken again.

A stopped sandbox is never started just for this. When something was saved, a line says so on stderr. When the fetch fails, the command still goes on, and a warning says that anything committed since the last fetch is only in the sandbox, with the `sbxm fetch` command to retry. Uncommitted changes are not fetched; commit them first.

## What rebuild and destroy protect

The bundle inside the sandbox disappears with the sandbox. So for a project added with `--local`, [`rebuild`](../../reference/cli/rebuild/) and [`destroy`](../../reference/cli/destroy/) count a commit as kept only when the host repository reaches it: from one of its branches or tags, or from what `sbxm fetch` saved under `refs/sbx/<sandbox>/`. A commit the host does not have stops them. An interactive terminal offers to fetch it and continue.

Uncommitted changes stop them the same way as for a GitHub project. Commit them in the sandbox first.

## Rebuilding and recovering

`sbxm rebuild` recreates the sandbox from the host repository. The same restoring happens whenever `open` or `repair` builds the sandbox again for a project that was built before, for example after the sandbox was removed outside sbxm. The first build of a newly added project restores nothing, even when a project of the same name added earlier left saved branches on the host. After the new sandbox has read its origin, sbxm sends the branches saved under `refs/sbx/<sandbox>/heads/` as one more bundle, brings them back as sandbox branches of the same names, and removes that bundle. A restored branch tracks `origin/<branch>` when the host has a branch of that name. The worktree on the start branch is then recreated at the saved tip, so work continues where it was fetched. Detached worktrees start from `origin/<branch>` as usual; the heads they had are still on the host under `refs/sbx/<sandbox>/worktrees/`. The result lists the branches that came back. A sandbox repository that already has branches is taken over as it is, and nothing is restored into it; one that stopped before any branch came back gets them on the next `open` or `repair`.
