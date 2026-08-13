---
title: '`sbxm ls`'
description: List managed projects and unmanaged sandboxes with their states.
---

```text
sbxm ls
```

`ls` reads the registry and the available Docker Sandboxes, then pairs the observed state with each registered project. It lists Docker Sandboxes that do not belong to a managed project separately. A project directory that moved is shown as `missing`; sbxm does not infer its new location.

For a managed project, `STATE` and `WORKSPACE` report two separate facts. `STATE` is the state of the record the sandbox runtime holds. `WORKSPACE` is whether the [neutral workspace directory](../../filesystem/#neutral-workspace) that record names is on the host, observed on every listing: `ready`, `missing`, `not-observed`, or `not-applicable` when the project has no sandbox yet. A project can be `stopped` and `missing` at the same time, which means it is not running and cannot start until the directory is restored.

The absence of a workspace directory is a state rather than an error, so it neither hides the rest of the listing nor changes the exit code. `ls` exits non-zero only when an entry and its artifacts disagree. For the reason behind a `not-observed` cell, run `sbxm status <project-id>`.

For an unmanaged sandbox, `STATE` and `WORKSPACE` show what the runtime reported, unmapped: sbxm declares nothing about a sandbox it does not manage.

See [Status values](../../status-values/) for the meaning and next action for
the state values shared with `status`.
