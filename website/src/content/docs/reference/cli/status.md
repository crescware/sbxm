---
title: '`sbxm status`'
description: Show the status of the host environment or one project without changing it.
---

```text
sbxm status --global
sbxm status <project-id>
```

Specify exactly one scope. `--global` checks the supported macOS platform, required commands, Docker Engine, Docker Sandboxes login and network policy, daemon state, and Remote SSH setup. A project ID checks its registry entry, host artifacts, image, sandbox, secret, repository, and worktrees.

Status is read-only and is the first command to run when another lifecycle command refuses to continue.
