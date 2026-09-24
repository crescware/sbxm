---
title: '`sbxm rebuild`'
description: Rebuild a project’s sandbox from its Dockerfile, whether or not it changed.
---

```text
sbxm rebuild [<project-id>]
```

Rebuild applies the host-side Dockerfile by recreating the image and sandbox, then restoring the repository setup and managed worktrees. The old sandbox writable layer is lost, whether or not the Dockerfile changed. In an interactive terminal, omit the project ID to choose a managed project, then type the project ID to confirm the protected plan. The prompt shows what to type, and asks again, up to three times in all, when the answer names something else. In a non-interactive terminal, normal rebuild refuses even when an explicit project ID is provided because that confirmation cannot be completed.

The normal command protects work by refusing dirty or untracked files, unpublished commits, in-progress Git operations, active sessions, unmanaged worktrees, and repository-level refs that cannot be recovered. A commit counts as published when the origin or the host repository can reach it: commits saved by [`sbxm fetch`](../fetch/) are kept. When unpublished commits are the only reason for the refusal, an interactive terminal offers to save them into the host repository and prepare the rebuild again; choosing to stop changes nothing. For a project added with `--local`, a running sandbox's commits are saved into the host repository first, and the branches saved on the host come back as sandbox branches in the new sandbox; see [Develop without GitHub](../../../guides/local-repository/#rebuilding). Before trusting anything reported from inside a running sandbox, it also confirms the sandbox's [neutral workspace directory](../../filesystem/#neutral-workspace) is still on the host, and refuses rather than risk reading a command that could not run as a sandbox with nothing to protect. See [Customize the sandbox image](../../../guides/custom-image/) for the workflow.
