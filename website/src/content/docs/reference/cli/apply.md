---
title: '`sbxm apply`'
description: Apply declared files or add managed worktrees without rebuilding a sandbox.
---

```text
sbxm apply <project-id> [--files] [--worktrees N]
```

At least one scope is required.

| Option | Meaning |
| --- | --- |
| `--files` | Re-place files declared in the global configuration, overwriting destinations |
| `--worktrees`, `-t` `N` | Set the project’s managed worktree count, from 1–32 |

The worktree count can increase but not decrease. Removing a worktree is not performed as a side effect because its contents may need preservation.
