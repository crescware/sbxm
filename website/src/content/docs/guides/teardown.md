---
title: Tear down safely
description: Understand what sbxm destroy removes, what it keeps, and when force is appropriate.
---

Destroy removes the sandbox and ends sbxm management for a project:

```sh
sbxm destroy <project-id>
```

Before deleting anything, sbxm shows what will be removed and what remains. It checks dirty worktrees, unpublished commits, repository-level refs, active sessions, and other conditions that could make data disappear unexpectedly.

## Removed

- the Docker Sandbox
- the project metadata used by sbxm
- the `GH_TOKEN` custom secret registered for that sandbox

Removing the custom secret matters: a stale registration can make the next registration fail as a duplicate and can leave a token attached to a sandbox that no longer exists.

## Kept

- the host clone
- the project Dockerfile
- built images
- loaded templates
- secrets registered for other sandboxes

Because sbxm never adopts an unregistered directory, registering the project again in the same place may require moving the old project directory aside first.

## Force

```sh
sbxm destroy --force <project-id>
```

`--force` skips data-protection and active-session checks and does not prompt for confirmation. Use it only after independently confirming that nothing inside the sandbox needs to be preserved.
