---
title: Configuration file
description: Configure Git identity defaults and files that sbxm places in a sandbox.
---

The global configuration lives at `~/.sbxm/config.yaml` and uses version 1:

```yaml
version: 1

files:
  - source: /Users/you/.gitconfig
    destination: .gitconfig
```

The file can also store the display language and default project Git identity chosen during the first interactive `add`. A project records the identity used when it was registered; changing the global default does not rewrite existing projects.

Destinations must be relative to the sandbox user’s home directory. sbxm rejects unsafe paths, unreadable declarations, and conflicting content instead of guessing what the declaration meant.

Use `sbxm apply <project-id> --files` to apply a later declaration change.
