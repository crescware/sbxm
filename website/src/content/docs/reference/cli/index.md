---
title: CLI reference
description: The sbxm command surface, project IDs, and lifecycle order.
---

sbxm manages a project from registration through teardown. The command surface is intentionally explicit: a command stops when it cannot establish the ownership or safety condition it needs.

## Lifecycle

| Command | Purpose |
| --- | --- |
| [`add`](./add/) | Register a GitHub repository and create host artifacts |
| [`prepare`](./prepare/) | Build and provision its Docker Sandbox |
| [`open`](./open/) | Start the sandbox and connect over SSH |
| [`ls`](./ls/) | List managed projects and sandbox state |
| [`status`](./status/) | Diagnose the host or one project, read-only |
| [`apply`](./apply/) | Apply declared files or add managed worktrees |
| [`rebuild`](./rebuild/) | Recreate a sandbox from its Dockerfile |
| [`stop`](./stop/) | Stop one or more sandboxes |
| [`destroy`](./destroy/) | Delete a sandbox and end management |

Global options are documented in [Global options](./global-options/). A project ID is the `owner/repository` identifier sbxm stores when a repository is registered.
