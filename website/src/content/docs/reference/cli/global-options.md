---
title: Global options
description: Options available on sbxm commands regardless of their lifecycle action.
---

| Option | Meaning |
| --- | --- |
| `--lang` `LANG` | Display language for the current run |
| `--color` `auto\|always\|never` | Color policy for each output stream |
| `--verbose` | Trace every external command and step with its elapsed time on standard error |
| `--help`, `-h` | Print help |
| `--version`, `-V` | Print the installed sbxm version |

`--lang`, `--color` and `--verbose` can be placed before or after a subcommand. `--verbose` has no short form; see [Output and color](/reference/output/) for what it prints. `--color=auto` colors a stream only when it is a terminal. `NO_COLOR`, `CLICOLOR_FORCE`, and `TERM=dumb` affect the default, while an explicit `--color` wins.
