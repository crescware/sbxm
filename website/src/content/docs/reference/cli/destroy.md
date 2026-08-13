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

Destroy does not need the [neutral workspace directory](../../filesystem/#neutral-workspace), so its absence neither blocks a deletion nor changes what destroy reports. A stopped sandbox is still refused in normal mode for the same reason as always: sbxm cannot look inside a sandbox that is not running.

Destroy does not delete that directory itself, and once a project has no sandbox record, sbxm reports its workspace as `not-applicable` rather than describing a directory it no longer maps to a record.

See [Tear down safely](../../../guides/teardown/) for the deletion and retention matrix.
