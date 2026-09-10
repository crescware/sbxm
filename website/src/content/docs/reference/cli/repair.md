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

Before changing anything, repair shows the observed project and sandbox, the target Dockerfile
generation, and the steps it may perform. When no artifact exists for an interrupted image build,
it can adopt a corrected Dockerfile; once an artifact exists, it keeps the recorded generation. It refuses to evict an active session, adopt an
ambiguous image or template, overwrite an artifact it cannot verify, or use a changed declared
configuration file as if it were the original input.

Repair reads a sandbox only while it is running. Looking inside a stopped sandbox would start
it, so a project whose sandbox is stopped is refused with what was observed instead of being
repaired from steps that were never read. Open the project to start it, then run `repair` if a
diagnostic still points to it.

A pending initial provisioning is resumed by `open` before it connects. `repair` remains an explicit
connection-free entry to the same recorded recovery when you want to prepare first. A successful repair verifies the resulting sandbox and clears
the initial-provisioning intent from project metadata only after that read-only verification
passes.
