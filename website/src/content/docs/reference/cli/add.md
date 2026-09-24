---
title: '`sbxm add`'
description: Add a GitHub repository, or a Git repository already on this host, to sbxm.
---

```text
sbxm add <github-clone-url> [options]
sbxm add --local <path> [--name <name>] [options]
```

Accepted repository URLs are the SSH and HTTPS GitHub clone URLs shown by GitHub. The transport you pass is used for the host clone.

With `--local`, the repository at `<path>` is added where it is, without a clone. The path must be the top of a Git working tree; a symlink is resolved and the real path is recorded. The project ID is `local/<name>`, where the name is the directory name unless `--name` gives another one. See [Develop without GitHub](../../../guides/local-repository/).

## Options

| Option | Meaning |
| --- | --- |
| `--local` `PATH` | Add the Git repository at `PATH` on this host instead of a GitHub repository |
| `--name` `NAME` | Name the project added with `--local`; defaults to the directory name |
| `--worktrees`, `-t` `N` | Set a target of 1–32 managed worktrees for the sandbox |
| `--detach` `BRANCH` | Start managed worktrees detached from a remote branch |
| `--git-user-name` `NAME` | Set the project commit name; provide it with email |
| `--git-user-email` `EMAIL` | Set the project commit email; provide it with name |
| `--lang` `LANG` | Choose the display language for this run |
| `--color` `MODE` | Choose `auto`, `always`, or `never` |

Without `--detach`, the first worktree follows the repository default branch; a repository added with `--local` starts from the branch it is on, and a detached one needs `--detach`. More than one worktree in detached mode requires an explicit starting branch.

## What it changes

It creates `<repository>.project/` in the current directory, a host clone, project metadata, and a Dockerfile. It prints the project ID, sandbox name, and credential command. It does not build the sandbox. A repository added with `--local` gets no host clone and needs no credential.

The first interactive registration asks for display language and Git identity. Non-interactive use must provide both identity options or an already saved default.
