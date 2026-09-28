---
title: Design principles
description: The safety and ownership rules that guide sbxm behavior.
---

sbxm treats ambiguity as dangerous. When external state, persistent state, or user intent cannot be determined uniquely, it does not continue a mutation by guessing.

## Do not infer ownership

sbxm does not adopt, overwrite, move, or delete an artifact just because its name resembles the expected project. Registry entries, paths, labels, origins, sandbox identities, and worktrees must agree.

## Observe before mutating

Validation happens before project state changes. An unobservable external condition is not treated as absence, equality, or safety.

## Make recovery explicit

When a safe condition is not met, the error reports the observed fact and points to a deliberate next action. Automatic repair is a separate workflow, not a hidden convenience of a normal command. As an exception, `open` may complete connection prerequisites — using a saved intent, a recorded baseline, or artifacts it could observe — only when they determine the exact target and change; ambiguous artifacts are never adopted.

## Keep the sandbox safe; favor convenience on the host only when unambiguous

sbxm does not change the work inside a sandbox — its branches, worktrees, and `HEAD` — from the host, unless the user confirmed that as a destructive operation. An agent may be working in the sandbox without the user who runs a command being there. On the host, sbxm may choose convenience only when the same operation done by hand would go through with Git or the tool concerned without asking anything and without a conflict; the user who runs the command is at the host and expects it to move. Each feature's specification states that condition.
