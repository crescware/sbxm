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

Register the token for the `github` service of Docker Sandboxes, scoped to the project's sandbox (`sbx secret set github --sandbox <sandbox>`). A sandbox-scoped registration takes effect at once, so the sandbox does not have to be rebuilt. If GitHub then rejects the credential, sbxm says so before the fetch and shows the commands that register the token anew.
