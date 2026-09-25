---
title: CLI reference
description: The sbxm command surface, project IDs, and lifecycle order.
---

sbxm manages a project from registration through teardown. The command surface is intentionally explicit: a command stops when it cannot establish the ownership or safety condition it needs.

Commands that use Docker Sandboxes (`open`, `apply`, `fetch`, `files pull`, `repair`, `rebuild`, `stop`, `destroy`, `ls`, and project or interactive `status`) check authentication before project selection or confirmation. If sign-in is required, they report `sbx-login-missing` and show `sbx login`. An explicit project ID or `destroy --force` also requires this check. `add` does not require Docker Sandboxes login; `status --global` reports login alongside the other host checks even when signed out.

## Lifecycle

| Command | Purpose |
| --- | --- |
| [`add`](./add/) | Add a GitHub repository to sbxm and clone it onto this host, or add a Git repository already on this host with `--local` |
| [`repair`](./repair/) | Explicitly recover an interrupted or incomplete initial provisioning |
| [`open`](./open/) | Open an SSH session to a project sandbox, building it on the first run and starting it if needed |
| [`ls`](./ls/) | List managed projects and unmanaged sandboxes with their states |
| [`status`](./status/) | Show host or project status without changing it |
| [`apply`](./apply/) | Apply declared files or add managed worktrees |
| [`fetch`](./fetch/) | Save a project sandbox's commits into its host repository without touching your branches |
| [`files`](./files/) | Declare host files to place in every sandbox, list them, or remove a declaration |
| [`rebuild`](./rebuild/) | Rebuild a project sandbox from its Dockerfile; the old writable layer is lost |
| [`stop`](./stop/) | Stop one or more project sandboxes without deleting them |
| [`destroy`](./destroy/) | Destroy a project sandbox and stop managing the project |

Global options are documented in [Global options](./global-options/). A project ID is the `owner/repository` identifier sbxm stores when a repository is registered.
