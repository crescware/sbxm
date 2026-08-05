---
title: '`sbxm open`'
description: Open an SSH session to a project sandbox, starting it if needed.
---

```text
sbxm open [<project-id>] [--index N]
```

If the sandbox is stopped, `open` starts it and then connects over SSH. In an interactive terminal, omit the project ID to choose a managed project. In a non-interactive terminal, an explicit project ID is required.

By default, the SSH session starts in `/home/agent/work/<repository>`. Use `--index`, or `-i`, with a zero-based managed worktree index to start in that worktree instead. If the index does not exist, sbxm warns and starts in the repository root.

| Option | Meaning |
| --- | --- |
| `--index`, `-i` `N` | Start in managed worktree `N` (zero-based) |

The project must already be prepared and the Docker Sandboxes Remote SSH integration must be configured on the host.
