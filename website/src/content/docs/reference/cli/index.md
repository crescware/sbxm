---
title: CLI reference
description: The sbxm command surface, project IDs, and lifecycle order.
---

sbxm manages a project from registration through teardown. The command surface is intentionally explicit: a command stops when it cannot establish the ownership or safety condition it needs.

## Lifecycle

| Command | Purpose |
| --- | --- |
| [`add`](./add/) | Add a GitHub repository to sbxm and clone it onto this host |
| [`prepare`](./prepare/) | Prepare a registered project by building and provisioning its sandbox |
| [`repair`](./repair/) | Continue an interrupted initial provisioning without deleting or overwriting project data |
| [`open`](./open/) | Open an SSH session to a project sandbox, starting it if needed |
| [`ls`](./ls/) | List managed projects and unmanaged sandboxes with their states |
| [`status`](./status/) | Show host or project status without changing it |
| [`apply`](./apply/) | Apply declared files or add managed worktrees |
| [`rebuild`](./rebuild/) | Rebuild a project sandbox from its Dockerfile; the old writable layer is lost |
| [`stop`](./stop/) | Stop one or more project sandboxes without deleting them |
| [`destroy`](./destroy/) | Destroy a project sandbox and stop managing the project |

Global options are documented in [Global options](./global-options/). A project ID is the `owner/repository` identifier sbxm stores when a repository is registered.
