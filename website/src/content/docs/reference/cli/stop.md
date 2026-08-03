---
title: '`sbxm stop`'
description: Stop one or more managed sandboxes without deleting them.
---

```text
sbxm stop [<project-id> ...]
```

Pass one or more project IDs to stop them. In an interactive terminal, you can omit the IDs and select projects. Stopping preserves the registration, host clone, Dockerfile, and sandbox data; use [`sbxm open`](../open/) to connect again.
