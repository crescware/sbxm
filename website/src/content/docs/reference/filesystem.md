---
title: Files and directories
description: Where sbxm stores project artifacts, registry state, and sandbox worktrees.
---

## Host project

Registering a repository from a parent directory creates:

```text
<parent>/<repository>.project/
├── <repository>/       # host-side clone
└── .sbxm/              # metadata, Dockerfile, lock, and cache
```

## Global state

```text
~/.sbxm/
├── registry.yaml       # registered project IDs and locations
└── config.yaml         # identity, language, and declared files when configured
```

The registry is the only place that knows where a project lives. Moving the project directory makes `sbxm ls` report `missing`; it does not adopt a matching directory by name.

## Neutral workspace

Every sandbox mounts one directory from the host:

```text
/tmp/docker-sandboxes/<sandbox-name>/
```

The path is derived from the project ID alone, so it carries neither the project directory nor your home directory into the sandbox. It is an empty mount point: the repository and the managed worktrees live inside the sandbox, not here.

The runtime records this path when the sandbox is created, and it refuses to start a sandbox whose recorded directory is no longer on the host. Because `/tmp` is cleared by periodic cleanup and by a restart, a sandbox that has been stopped for a while can lose its workspace directory while its record remains.

That is a missing start-up condition rather than lost data. `sbxm ls` reports it in the `WORKSPACE` column, and `sbxm status <project-id>` reports it as the `workspace` item; both use `missing` for a directory observed to be absent. Preparing the project again creates the directory and says that it did.

## Sandbox worktrees

Inside a prepared sandbox, managed worktrees look like:

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
```

The exact count and attached/detached mode come from the project registration and later `apply` changes.
