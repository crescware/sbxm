---
title: Troubleshooting
description: Start with read-only status checks when sbxm cannot continue.
---

sbxm refuses a mutation when ownership, external state, or user intent cannot be established safely. Start with the narrowest read-only diagnosis:

1. Run [`sbxm status --global`](../reference/cli/status/) for host and Docker Sandboxes problems.
2. Run [`sbxm status <project-id>`](../reference/cli/status/) for project artifacts, credentials, repositories, and worktrees.
3. Follow the relevant [safety refusal](./safety-refusals/) guidance.
4. Re-run the original command after the observed condition is fixed.

## Choose a scope

- [Diagnose the host](./host/) — platform, Docker, login, network, and SSH.
- [Diagnose a project](./project/) — registry, image, sandbox, secret, repository, and worktree.
- [Resolve safety refusals](./safety-refusals/) — dirty work, collisions, active sessions, and ownership.
