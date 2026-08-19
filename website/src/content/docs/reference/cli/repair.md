---
title: '`sbxm repair`'
description: Repair interrupted initial provisioning without deleting or overwriting project data.
---

```text
sbxm repair [<project-id>]
```

`repair` explicitly continues an interrupted first provisioning. It is the only workflow that may resume an initial provisioning after `prepare` has stopped partway through. A normal `prepare` refuses to continue once an interrupted provisioning intent has been recorded.

Before changing anything, repair observes the registered project, sandbox, image, repository, declared files, credentials, and managed worktrees. It prints the recorded target generation and the artifacts it found. If the ownership and safety checks cannot be proven, it stops without changing the project.

When the checks pass, repair completes the same provisioning steps as `prepare`: it creates or verifies the sandbox, applies declared files, configures the Git identity and credential helper, clones the repository, and creates the managed worktrees. It does not adopt, delete, or overwrite project data. The recorded intent is cleared only after the final validation succeeds, so an interruption can be repaired by running the command again.

In an interactive terminal, omit the project ID to choose a registered project. In a non-interactive terminal, an explicit project ID is required.

Repair is specifically for interrupted initial provisioning. Use [`prepare`](../prepare/) for a fresh project, [`open`](../open/) when the project is ready, and [`status`](../status/) to inspect a project without changing it.
