---
title: '`sbxm repair`'
description: Explicitly recover an interrupted or incomplete initial provisioning.
---

```text
sbxm repair [<project-id>]
```

Repair is the explicit recovery workflow for a project whose initial provisioning was
interrupted or left incomplete. In an interactive terminal, omit the project ID to choose a
managed project. In a non-interactive terminal, an explicit project ID is required.

Before changing anything, repair shows the observed project and sandbox, the fixed Dockerfile
generation, and the steps it may perform. It refuses to evict an active session, adopt an
ambiguous image or template, overwrite an artifact it cannot verify, or use a changed declared
configuration file as if it were the original input.

`prepare` never resumes a pending or incomplete initial provisioning implicitly. Run `repair`
when the diagnostic points to it. A successful repair verifies the resulting sandbox and clears
the initial-provisioning intent from project metadata only after that read-only verification
passes.
