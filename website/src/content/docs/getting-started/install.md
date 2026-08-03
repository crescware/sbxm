---
title: Install sbxm
description: Install the sbxm CLI on a supported Apple silicon Mac.
---

Install the current Homebrew formula:

```sh
brew install crescware/tap/sbxm
```

Confirm that the command is available:

```sh
sbxm --version
sbxm status --global
```

The first command prints the installed sbxm version. The second checks the host and Docker Sandboxes environment without creating or changing a project.

## Next step

Once the global status is healthy, follow [Create your first sandbox](../quickstart/).

Release archives and checksums are published on [GitHub Releases](https://github.com/crescware/sbxm/releases). Homebrew is the supported installation path for the CLI.
