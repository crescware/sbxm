---
title: '`sbxm destroy`'
description: Destroy a project sandbox and end sbxm management while keeping its host clone.
---

```text
sbxm destroy [<project-id>]
sbxm destroy --force <project-id>
```

Normal destroy checks dirty worktrees, unpublished commits, repository-level refs, active sessions, and ownership before showing what will be removed and what remains. A commit counts as published when the origin or the host repository can reach it: commits saved by [`sbxm fetch`](../fetch/) are kept. When unpublished commits on branches, tags, or worktree `HEAD`s are the only reason for the refusal, an interactive terminal offers to save them into the host repository and prepare the teardown again; choosing to stop changes nothing. A stash or notes commit is not saved this way. For a project added with `--local`, a running sandbox's commits are saved into the host repository first, even with `--force`. In an interactive terminal, omit the ID to select a project, then type the project ID — the same `<owner>/<repository>` you pass on the command line — to confirm the protected plan. The prompt shows what to type, and asks again, up to three times in all, when the answer names something else; the sandbox name shown in the plan is accepted as well, and case is not significant. In a non-interactive terminal, normal destroy refuses rather than skipping that confirmation.

`--force`, or `-f`, is the only non-interactive bypass: it skips data-protection and active-session checks and does not prompt for confirmation. It does not make data recoverable. Use it only when you have independently confirmed that the sandbox contains nothing to preserve.

Removing a sandbox record does not itself touch the [neutral workspace directory](../../filesystem/#neutral-workspace). Normal mode does read it once, though: before removing a *running* sandbox, destroy confirms the directory is still on the host as part of the same check that looks for dirty worktrees and unpublished commits, and refuses instead of removing the sandbox if it is not. A stopped sandbox is started first: sbxm cannot look inside a sandbox that is not running, and a plan drawn without looking would show an empty list of losses as though there were none. That start is the only host change normal destroy makes before the confirmation. It prepares nothing else — no fetch, no worktrees — so a project you are about to delete is never rebuilt on the way out, and cancelling the confirmation leaves the sandbox running. `--force` skips this check along with every other data-protection check, and starts nothing.

Destroy does not delete that directory itself, and once a project has no sandbox record, sbxm reports its workspace as `not-applicable` rather than describing a directory it no longer maps to a record.

See [Tear down safely](../../../guides/teardown/) for the deletion and retention matrix.
