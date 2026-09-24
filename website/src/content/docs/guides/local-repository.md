---
title: Develop without GitHub
description: Use a Git repository on this host as the origin of a disposable sandbox.
---

A repository that already lives on this host can be a project. sbxm does not clone it. The repository's `.git` acts as the origin, and the sandbox is a disposable place to work in that you can recreate from the host.

```sh
cd ~/Projects
sbxm add --local ~/code/<repository>
sbxm open local/<repository>
```

## Registering

The path must be the top of a Git working tree. A symlink is resolved, and the real path is recorded. The project ID is `local/<name>`, where the name is the directory name unless you pass `--name <name>`. When the directory name cannot name a project, `add` asks for `--name`.

The sandbox starts from the branch the host repository is on. When the host repository is detached, pass the starting branch with `--detach`. No GitHub token is involved, so there is nothing to register before `open`.

## Getting the host's history into the sandbox

When `open` builds the sandbox, sbxm writes the host repository's branches and tags into one `git bundle`. It streams the bundle into the sandbox through the standard input of `sbx exec` and places it at `<repository>/.git/sbxm/origin.bundle` after checking its digest. The sandbox's `origin` points at that file, so the managed worktrees are created from `origin/<branch>` as they are for a GitHub project. Nothing in the sandbox can reach the host repository.

## Bringing work back

Run [`sbxm fetch`](../../reference/cli/fetch/) to save the sandbox's commits into `refs/sbx/<sandbox>/` of the host repository. Your branches are never touched; merge what you want yourself, for example:

```sh
git merge refs/sbx/<sandbox>/heads/main
```

## What rebuild and destroy protect

The bundle inside the sandbox disappears with the sandbox. So for a project added with `--local`, [`rebuild`](../../reference/cli/rebuild/) and [`destroy`](../../reference/cli/destroy/) count a commit as kept only when the host repository reaches it: from one of its branches or tags, or from what `sbxm fetch` saved under `refs/sbx/<sandbox>/`. A commit the host does not have stops them. An interactive terminal offers to fetch it and continue.

Uncommitted changes stop them the same way as for a GitHub project. Commit them in the sandbox first.
