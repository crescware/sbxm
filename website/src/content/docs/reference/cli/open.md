---
title: '`sbxm open`'
description: Start a project sandbox when necessary and connect over SSH.
---

```text
sbxm open [<project-id>]
```

If the sandbox is stopped, `open` starts it and then connects over SSH. In an interactive terminal, omit the project ID to choose a managed project. In a non-interactive terminal, an explicit project ID is required.

The project must already be prepared and the Docker Sandboxes Remote SSH integration must be configured on the host.
