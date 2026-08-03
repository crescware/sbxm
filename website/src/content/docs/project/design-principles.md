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

When a safe condition is not met, the error reports the observed fact and points to a deliberate next action. Automatic repair is a separate workflow, not a hidden convenience of a normal command.
