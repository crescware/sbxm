---
title: '`sbxm prepare`'
description: Prepare a registered project by building and provisioning its Docker Sandbox.
---

```text
sbxm prepare <project-id>
```

Prepare builds the project image, creates the sandbox, clones the repository inside it, applies declared files, and creates the configured managed worktrees.

Before running it, register the project-specific `GH_TOKEN` custom secret printed by [`sbxm add`](../add/). The secret proxy must cover the GitHub hosts the repository uses.

Prepare is a mutation. It refuses artifacts that cannot be proven to belong to the project instead of adopting or overwriting them.
