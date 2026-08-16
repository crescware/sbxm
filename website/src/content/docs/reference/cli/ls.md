---
title: '`sbxm ls`'
description: List managed projects and unmanaged sandboxes with their states.
---

```text
sbxm ls
```

`ls` reads the registry and the available Docker Sandboxes, then gives each registered project one user-facing `STATE`. It lists Docker Sandboxes that do not belong to a managed project separately. A project directory that moved is shown as `missing`; sbxm does not infer its new location.

For a managed project, `STATE` answers whether `sbxm open` can proceed directly. `running` means the sandbox is already running. `stopped` means the sandbox exists and `sbxm open` starts it, so no prior user action is needed. `open-blocked` means sbxm has observed a startup prerequisite that prevents `open` from completing directly; the reason and recovery action are shown by `sbxm status <project-id>` or by the diagnostic from `open`. `not-observed` means sbxm could not decide whether `open` can proceed.

The workspace directory remains an internal observation and is reported as the `workspace` item by `sbxm status <project-id>`. It is not a separate column in the managed-projects table.

The absence of a workspace directory is a state rather than an error, so it neither hides the rest of the listing nor changes the exit code. `ls` exits non-zero only when an entry and its artifacts disagree. For the reason behind a `not-observed` cell, run `sbxm status <project-id>`.

For an unmanaged sandbox, `STATE` and `WORKSPACE` show what the runtime reported, unmapped: sbxm declares nothing about a sandbox it does not manage.

See [Status values](../../status-values/) for the detailed observation values reported by
`status`.
