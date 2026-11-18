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

When Docker Sandboxes reports its 120-second pause on authentication refresh after waking, sbxm reads the host's wake time to estimate when the pause ends. With less than 30 seconds remaining, it shows `Connecting…`, waits only the remaining time, and checks once more with a 10-second timeout. Ctrl-C cancels the wait. With 30 seconds or more remaining, it exits with a brief explanation and the approximate local time to try again. If the wake time or local time cannot be read reliably, it shows the explanation without guessing a retry time. A failed check after waiting never starts another retry cycle. For background, see the [Docker Sandboxes v0.48.0 release notes](https://github.com/docker/sbx-releases/releases/tag/v0.48.0).
