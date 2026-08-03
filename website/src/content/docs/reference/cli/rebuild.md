---
title: '`sbxm rebuild`'
description: Rebuild a project’s sandbox from its edited Dockerfile.
---

```text
sbxm rebuild <project-id>
```

Rebuild applies the host-side Dockerfile by recreating the image and sandbox, then restoring the repository setup and managed worktrees.

The normal command protects work by refusing dirty files, unpushed commits, in-progress Git operations, active sessions, and unmanaged worktrees. See [Customize the sandbox image](../../../guides/custom-image/) for the workflow.
