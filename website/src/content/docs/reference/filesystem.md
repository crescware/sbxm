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

The runtime records this path when the sandbox is created, and it refuses to start a sandbox whose recorded directory is no longer on the host. This host-side `/tmp` path is volatile: it can disappear after a reboot, OS cleanup, or other external/manual removal while the sandbox record remains. It is separate from the sandbox's own `/tmp`, which is inside the writable layer and is lost when that layer is recreated.

That is a missing start-up condition rather than lost data. `sbxm ls` reports it in the `WORKSPACE` column, and `sbxm status <project-id>` reports it as the `workspace` item; both use `missing` for a directory observed to be absent. Preparing the project again creates the directory and says that it did.

A running sandbox can lose the same directory the same way, without being stopped first. The runtime does not refuse an existing session's commands just because the host-side source of a live mount vanished, and a command that then fails reports the same exit status whether the sandbox holds a repository or the directory is simply gone. Because of that, `sbxm destroy` and `sbxm rebuild` confirm the directory is on the host before trusting anything a running sandbox reports about what is inside it, and refuse rather than guess when it is not.

## Recover disk space

When a sandbox is full, use this order:

1. Delete unnecessary files inside the sandbox; the space returns immediately.
2. Understand what a rebuild discards. If recreating the writable layer is
   necessary, use the normal protected `sbxm rebuild` flow and review its plan.
3. Save or resolve every Layer A blocker before retrying, including unpublished
   commits, dirty or untracked work, in-progress Git operations, active
   sessions, and repository-level refs.

## Sandbox worktrees

Inside a prepared sandbox, managed worktrees look like:

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
```

The exact count and attached/detached mode come from the project registration and later `apply` changes.
