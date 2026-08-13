---
title: '`sbxm ls`'
description: List managed projects and unmanaged sandboxes with their states.
---

```text
sbxm ls
```

`ls` reads the registry and the available Docker Sandboxes, then pairs the observed state with each registered project. It lists Docker Sandboxes that do not belong to a managed project separately. A project directory that moved is shown as `missing`; sbxm does not infer its new location.

See [Status values](../../status-values/) for the meaning and next action for
the state values shared with `status`.
