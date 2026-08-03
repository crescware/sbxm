---
title: Output and color
description: Understand sbxm output streams and control terminal color.
---

sbxm writes results to standard output. Progress, prompts, warnings, and errors go to standard error, so redirecting a result does not mix diagnostics into it.

## Color options

| Setting | Effect |
| --- | --- |
| `--color=auto` | Color a stream only when it is a terminal (default) |
| `--color=always` | Color even a redirected stream |
| `--color=never` | Never color anything |
| `NO_COLOR` | Disable color for any value, including empty |
| `CLICOLOR_FORCE` | Enable color unless the value is `0` |
| `TERM=dumb` | Disable color and use ASCII markers |

An explicit `--color` wins over environment variables. Disabling color does not remove information: markers, labels, and blank lines keep the same meaning.
