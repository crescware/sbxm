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

When the observation points to exactly one thing to do next, status ends with that single command and the reason for it. It never offers two competing commands: a project whose first provisioning was interrupted or left incomplete is sent to `repair` even when its Dockerfile also changed, and running status again after the recovery reports whether a generation change is still needed. An unfinished first provisioning or an unfinished generation change ends with a non-zero exit status because the project has not reached its target configuration; a changed Dockerfile alone is not damage, so it is named while status still succeeds. When the safe action cannot be determined — an identity mismatch, an artifact that could not be observed, or a sandbox that is not running — status reports what it observed and offers no command.
See [Status values](../../status-values/) for the meaning and next action for each
state value.

In the project `WORKTREES` table, `STATE` describes the worktree itself and
`REMOTE` separately describes whether the current commit is pushed, reachable
from an origin ref, unreachable, or `unobservable(reason)`. Status does not
fetch; when its local refs or objects are insufficient, it keeps the result
unknown and explains the recovery action.
