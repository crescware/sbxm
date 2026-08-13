---
title: Diagnose a project
description: Use project status to inspect registered artifacts and sandbox state.
---

Run:

```sh
sbxm status <project-id>
```

Project status checks the registration and its path, project metadata, host clone and origin, Dockerfile and image, sandbox identity and location, custom secret, repository inside the sandbox, and managed worktrees.

When the `workspace` item is `missing`, the sandbox record still names a [neutral workspace directory](../../reference/filesystem/#neutral-workspace) that the host no longer has, and the runtime refuses to start the sandbox. This happens most often to a project that has been stopped for a while, because `/tmp` is cleared by periodic cleanup and by a restart. Nothing of the project is in that directory, so preparing the project again is enough: it creates the directory and reports the path it created.

When a project is reported as `missing`, inspect the registry path yourself. sbxm never adopts a directory solely because its name looks similar. When a secret or repository check fails, follow the credential command printed by `add` and confirm the secret was registered before the sandbox was created.
