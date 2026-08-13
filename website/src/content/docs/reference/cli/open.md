---
title: '`sbxm open`'
description: Open an SSH session to a project sandbox, starting it if needed.
---

```text
sbxm open [<project-id>] [--index N]
```

If the sandbox is stopped, `open` starts it and then connects over SSH. In an interactive terminal, omit the project ID to use one prompt: the up and down cursor keys choose a managed project, the left and right cursor keys adjust its zero-based worktree index, and one Enter confirms both. So it can appear immediately, the prompt opens before project metadata is read. Until that project's result arrives, the index line reads `(calculating)` rather than naming a range sbxm cannot yet know; the index still moves in the meantime. Metadata is calculated in the background, and when the result arrives the prompt shows that project's own range and holds the index within it. If the confirmed index is still above what the project declares under its lock, sbxm warns and opens the project's last managed worktree. In a non-interactive terminal, an explicit project ID is required.

When a project ID is supplied without `--index`, the SSH session starts in `/home/agent/work/<repository>`. Use `--index`, or `-i`, with a zero-based managed worktree index to start in that worktree instead. If the index does not exist, sbxm warns and starts in the repository root.

| Option | Meaning |
| --- | --- |
| `--index`, `-i` `N` | Start in managed worktree `N` (zero-based) |

Starting a stopped sandbox requires the [neutral workspace directory](../../filesystem/#neutral-workspace) its record names. `open` observes that directory before it asks the runtime to start, and refuses when the directory is absent or cannot be observed, naming the path and how to restore it. A sandbox that is already running is not started again and is not held back by that observation.

The project must already be prepared and the Docker Sandboxes Remote SSH integration must be configured on the host.
