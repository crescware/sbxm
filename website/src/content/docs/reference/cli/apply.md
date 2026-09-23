---
title: '`sbxm apply`'
description: Apply declared files or add managed worktrees without rebuilding a sandbox.
---

```text
sbxm apply [<project-id>] [--files [--force]] [--worktrees N]
```

At least one scope is required. In an interactive terminal, omit the project ID to choose a managed project. In a non-interactive terminal, an explicit project ID is required.

| Option | Meaning |
| --- | --- |
| `--files` | Re-place files declared in the global configuration, replacing only destinations that still hold what sbxm placed last |
| `--force` | With `--files`, also replace declared files that were edited inside the sandbox or that sbxm has no record of placing |
| `--worktrees`, `-t` `N` | Set the desired managed worktree count to 1–32 without lowering it |

Without `--force`, `--files` refuses before placing anything when a destination holds content sbxm did not place last, and names every such file. `--force` without `--files` is refused.

The worktree count can increase but not decrease. Removing a worktree is not performed as a side effect because its contents may need preservation.
