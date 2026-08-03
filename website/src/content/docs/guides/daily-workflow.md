---
title: Daily workflow
description: Inspect, open, stop, and manage registered sbxm projects.
---

Once a project is prepared, these commands cover the normal day-to-day loop.

## List projects

```sh
sbxm ls
```

`ls` shows every registered project and the state of its associated sandbox. A moved project is reported as `missing`; sbxm does not guess a new path.

## Inspect without changing state

```sh
sbxm status <project-id>
```

Project status checks the registry, host artifacts, sandbox identity, repository, credential registration, and managed worktrees without mutating them.

## Connect

```sh
sbxm open <project-id>
```

`open` starts the sandbox if needed and connects over SSH. In an interactive terminal you can omit the project ID and select a project. In a non-interactive terminal, pass the ID explicitly.

## Stop without deleting

```sh
sbxm stop <project-id>
sbxm stop <project-id> <another-project-id>
```

Stopping preserves the project registration and its sandbox. Start it again with `sbxm open`.

## Apply small changes

Add managed worktrees or re-place declared files without rebuilding the image:

```sh
sbxm apply <project-id> --worktrees 4
sbxm apply <project-id> --files
```

See [managed worktrees](./worktrees/) and [configuration files](./configuration-files/) for their safety rules.
