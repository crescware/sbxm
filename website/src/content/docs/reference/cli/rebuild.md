---
title: '`sbxm rebuild`'
description: Rebuild a project’s sandbox from its Dockerfile, whether or not it changed.
---

```text
sbxm rebuild [<project-id>]
```

Rebuild applies the host-side Dockerfile by recreating the image and sandbox, then restoring the repository setup and managed worktrees. The old sandbox writable layer is lost, whether or not the Dockerfile changed. In an interactive terminal, omit the project ID to choose a managed project. In a non-interactive terminal, an explicit project ID is required.

The normal command protects work by refusing dirty or untracked files, unpublished commits, in-progress Git operations, active sessions, unmanaged worktrees, and repository-level refs that cannot be recovered. Before trusting anything reported from inside a running sandbox, it also confirms the sandbox's [neutral workspace directory](../../filesystem/#neutral-workspace) is still on the host, and refuses rather than risk reading a command that could not run as a sandbox with nothing to protect. See [Customize the sandbox image](../../../guides/custom-image/) for the workflow.
