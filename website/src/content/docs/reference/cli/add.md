---
title: '`sbxm add`'
description: Register a GitHub repository and create its host-side sbxm artifacts.
---

```text
sbxm add <github-clone-url> [options]
```

Accepted repository URLs are the SSH and HTTPS GitHub clone URLs shown by GitHub. The transport you pass is used for the host clone.

## Options

| Option | Meaning |
| --- | --- |
| `--worktrees`, `-t` `N` | Create 1–32 managed worktrees |
| `--detach` `BRANCH` | Start managed worktrees detached from a remote branch |
| `--git-user-name` `NAME` | Set the project commit name; provide it with email |
| `--git-user-email` `EMAIL` | Set the project commit email; provide it with name |
| `--lang` `LANG` | Choose the display language for this run |
| `--color` `MODE` | Choose `auto`, `always`, or `never` |

Without `--detach`, the first worktree follows the repository default branch. More than one worktree in detached mode requires an explicit starting branch.

## What it changes

It creates `<repository>.project/` in the current directory, a host clone, project metadata, and a Dockerfile. It prints the project ID, sandbox name, and credential command. It does not build the sandbox.

The first interactive registration asks for display language and Git identity. Non-interactive use must provide both identity options or an already saved default.
