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

See [Tear down safely](../../../guides/teardown/) for the deletion and retention matrix.
