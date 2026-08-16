---
title: Resolve safety refusals
description: Understand common sbxm refusals and the safe next observation.
---

### Dirty or unpublished work

Commit and push what you want to keep, remove what you do not, and run status
again. Rebuild and destroy refuse to guess whether uncommitted or unreachable
work is disposable. A clean worktree does not cover repository-level local
branches, tags, notes, stash entries, extra remotes, or reflog-only commits;
save or resolve those Layer A blockers independently.

### Active session

Close the sandbox session, then retry the normal command. Use `--force` only for destroy when you have independently confirmed that no session work needs preservation.

### Unmanaged worktree

Inspect the worktree inside the sandbox and save or remove it yourself. Rebuild does not delete a worktree it cannot account for.

### Collision or mismatch

Inspect the existing path, image, Dockerfile, registry entry, or sandbox identity. sbxm does not overwrite or adopt an artifact that declares another project.

### Missing credential

Register the project-specific custom secret with Docker Sandboxes, then prepare a new sandbox if necessary. The secret must cover the GitHub hosts and `GH_TOKEN` environment variable the project expects.
