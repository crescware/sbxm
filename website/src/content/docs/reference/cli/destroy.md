---
title: '`sbxm destroy`'
description: Delete a project sandbox and end sbxm management with data-protection checks.
---

```text
sbxm destroy [<project-id>] [--force]
```

Normal destroy checks dirty worktrees, unpushed commits, active sessions, and ownership before showing what will be removed and what remains. In an interactive terminal, omit the ID to select a project.

`--force`, or `-f`, skips data-protection and active-session checks. It does not make data recoverable. Use it only when you have independently confirmed that the sandbox contains nothing to preserve.

See [Tear down safely](/guides/teardown/) for the deletion and retention matrix.
