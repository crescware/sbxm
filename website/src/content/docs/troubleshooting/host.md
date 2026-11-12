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

If a command reports `sbx-login-missing`, run `sbx login`, then retry it. Commands that use Docker Sandboxes check this before displaying the project selection prompt. `sbx-login-unobservable` means `sbx ls --json` failed, so sbxm could not check authentication; it does not establish that you are signed out. The diagnostic includes the failed command, the reason, and any stderr received before a timeout. Check your network connection and, after waking the host or updating Docker Sandboxes, wait briefly and retry. If the problem persists, run `sbx diagnose`. `sbxm status --global` can still diagnose the host while signed out.

Immediately after the host wakes from sleep, Docker Sandboxes may pause authentication refresh, preventing commands from running. If you see this notice, retry the command once about two minutes have passed since the host woke up. For related information, see the [Docker Sandboxes v0.48.0 release notes](https://github.com/docker/sbx-releases/releases/tag/v0.48.0).
