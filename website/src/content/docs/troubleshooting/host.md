---
title: Diagnose the host
description: Use global status to inspect the machine and Docker Sandboxes environment.
---

Run:

```sh
sbxm status --global
```

The check is read-only and covers:

- macOS version and arm64 platform
- required commands such as Docker, Git, SSH, and `sbx`
- Docker Engine reachability
- Docker Sandboxes CLI version, login, daemon state, and network policy
- Remote SSH configuration for sandbox connections

Fix the reported prerequisite, then run global status again. sbxm does not infer that an unobservable requirement is safe.
