---
title: '`sbxm open`'
description: Open an SSH session to a project sandbox, starting it if needed.
---

```text
sbxm open [<project-id>]
```

If the sandbox is stopped, `open` starts it and then connects over SSH. In an interactive terminal, omit the project ID to choose a managed project. In a non-interactive terminal, an explicit project ID is required.

The project must already be prepared and the Docker Sandboxes Remote SSH integration must be configured on the host.
