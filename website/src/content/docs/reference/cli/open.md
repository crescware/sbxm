---
title: '`sbxm open`'
description: Open an SSH session to a project sandbox, building it on the first run and starting it if needed.
---

```text
sbxm open [<project-id>] [--index N]
```

If the project has no sandbox yet, the first `open` builds one in the same command: it pins the Dockerfile and declared files it is about to use, records a first-provisioning intent before the first change, builds the image, creates the sandbox, clones the repository inside it, creates the managed worktrees, and then connects. If a step is interrupted, the intent stays on disk and the next `open` resumes from immutable recorded inputs. Completed images, templates, sandboxes, repositories, and worktrees are verified and reused. If the sandbox is stopped, `open` starts it and then connects over SSH. In an interactive terminal, omit the project ID to use one prompt: the up and down cursor keys choose a managed project, the left and right cursor keys adjust its zero-based worktree index, and one Enter confirms both. So it can appear immediately, the prompt opens before project metadata is read. Until that project's result arrives, the index line reads `(calculating)` rather than naming a range sbxm cannot yet know; the index still moves in the meantime. Metadata is calculated in the background, and when the result arrives the prompt shows that project's own range and holds the index within it. If the confirmed index is still above what the project declares under its lock, sbxm warns and opens the project's last managed worktree. In a non-interactive terminal, an explicit project ID is required.

When a project ID is supplied without `--index`, the SSH session starts in `/home/agent/work/<repository>`. Use `--index`, or `-i`, with a zero-based managed worktree index to start in that worktree instead. If the index does not exist, sbxm warns and starts in the repository root.

| Option | Meaning |
| --- | --- |
| `--index`, `-i` `N` | Start in managed worktree `N` (zero-based) |

Starting a stopped sandbox requires the [neutral workspace directory](../../filesystem/#neutral-workspace) its record names. `open` verifies the sandbox identity, restores that private empty mount point when it is absent, reports the restoration, and then starts the sandbox. A sandbox that is already running is not started again.

The Docker Sandboxes Remote SSH integration must be configured on the host.
