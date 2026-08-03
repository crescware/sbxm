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

## Sandbox worktrees

Inside a prepared sandbox, managed worktrees look like:

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
```

The exact count and attached/detached mode come from the project registration and later `apply` changes.
