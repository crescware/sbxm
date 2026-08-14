---
title: '`sbxm destroy`'
description: Destroy a project sandbox and end sbxm management while keeping its host clone.
---

```text
sbxm destroy [<project-id>]
sbxm destroy --force <project-id>
```

Normal destroy checks dirty worktrees, unpushed commits, active sessions, and ownership before showing what will be removed and what remains. In an interactive terminal, omit the ID to select a project.

`--force`, or `-f`, skips data-protection and active-session checks and does not prompt for confirmation. It does not make data recoverable. Use it only when you have independently confirmed that the sandbox contains nothing to preserve.

Removing a sandbox record does not itself touch the [neutral workspace directory](../../filesystem/#neutral-workspace). Normal mode does read it once, though: before removing a *running* sandbox, destroy confirms the directory is still on the host as part of the same check that looks for dirty worktrees and unpushed commits, and refuses instead of removing the sandbox if it is not. A stopped sandbox is refused in normal mode regardless, for the reason it always has been: sbxm cannot look inside a sandbox that is not running. `--force` skips this check along with every other data-protection check.

Destroy does not delete that directory itself, and once a project has no sandbox record, sbxm reports its workspace as `not-applicable` rather than describing a directory it no longer maps to a record.

See [Tear down safely](../../../guides/teardown/) for the deletion and retention matrix.
