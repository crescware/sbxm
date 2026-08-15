---
title: '`sbxm status`'
description: Show the status of the host environment or one project without changing it.
---

```text
sbxm status [<project-id>]
sbxm status --global
```

In an interactive terminal, omitting the project ID opens a selection prompt. The first choice is `global`, followed by registered project IDs. In a non-interactive terminal, specify exactly one scope. `--global` checks the supported macOS platform, required commands, Docker Engine, Docker Sandboxes login and network policy, daemon state, and Remote SSH setup. A project ID checks its registry entry, host artifacts, image, sandbox, secret, repository, and worktrees.

Status is read-only and is the first command to run when another lifecycle command refuses to continue.
See [Status values](../../status-values/) for the meaning and next action for each
state value.

In the project `WORKTREES` table, `STATE` describes the worktree itself and
`REMOTE` separately describes whether the current commit is pushed, reachable
from an origin ref, unreachable, or `unobservable(reason)`. Status does not
fetch; when its local refs or objects are insufficient, it keeps the result
unknown and explains the recovery action.
