---
title: '`sbxm open`'
description: Open an SSH session to a project sandbox, starting it if needed.
---

```text
sbxm open [<project-id>] [--index N]
```

If the sandbox is stopped, `open` starts it and then connects over SSH. In an interactive terminal, omit the project ID to use one prompt: the up and down cursor keys choose a managed project, the left and right cursor keys adjust its zero-based worktree index, and one Enter confirms both. Before project metadata is read, the prompt accepts optimistic indices `0`–`31` so it can appear immediately. Metadata is calculated in the background; when the selected project's result arrives, sbxm updates the displayed maximum and clamps the index to the selected project's actual worktree count. In a non-interactive terminal, an explicit project ID is required.

When a project ID is supplied without `--index`, the SSH session starts in `/home/agent/work/<repository>`. Use `--index`, or `-i`, with a zero-based managed worktree index to start in that worktree instead. If the index does not exist, sbxm warns and starts in the repository root.

| Option | Meaning |
| --- | --- |
| `--index`, `-i` `N` | Start in managed worktree `N` (zero-based) |

The project must already be prepared and the Docker Sandboxes Remote SSH integration must be configured on the host.
