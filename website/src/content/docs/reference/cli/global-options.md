---
title: Global options
description: Options available on sbxm commands regardless of their lifecycle action.
---

| Option | Meaning |
| --- | --- |
| `--lang` `LANG` | Display language for the current run |
| `--color` `auto\|always\|never` | Color policy for each output stream |
| `--help`, `-h` | Print help |
| `--version`, `-V` | Print the installed sbxm version |

`--lang` and `--color` can be placed before or after a subcommand. `--color=auto` colors a stream only when it is a terminal. `NO_COLOR`, `CLICOLOR_FORCE`, and `TERM=dumb` affect the default, while an explicit `--color` wins.
