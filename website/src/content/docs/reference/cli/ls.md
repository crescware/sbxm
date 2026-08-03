---
title: '`sbxm ls`'
description: List registered projects and their Docker Sandbox state.
---

```text
sbxm ls
```

`ls` reads the registry and the available Docker Sandboxes, then pairs the observed state with each registered project. A project directory that moved is shown as `missing`; sbxm does not infer its new location.
