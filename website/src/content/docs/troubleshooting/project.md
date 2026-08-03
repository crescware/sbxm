---
title: Diagnose a project
description: Use project status to inspect registered artifacts and sandbox state.
---

Run:

```sh
sbxm status <project-id>
```

Project status checks the registration and its path, project metadata, host clone and origin, Dockerfile and image, sandbox identity and location, custom secret, repository inside the sandbox, and managed worktrees.

When a project is reported as `missing`, inspect the registry path yourself. sbxm never adopts a directory solely because its name looks similar. When a secret or repository check fails, follow the credential command printed by `add` and confirm the secret was registered before the sandbox was created.
