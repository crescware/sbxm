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

## Verbose trace

`--verbose` adds a trace to standard error: one dim line when sbxm starts an external command and one when it ends, one line for each step once the next step begins, and a total when the run ends. Every elapsed time is right-aligned at the end of its line, in milliseconds with three decimals, so `sort` and `grep` keep working.

```text
→ Checking the sandbox state. Waiting for Docker Sandboxes to respond.
run    sbx ls --json
done   sbx ls --json: exit status: 0, stdout 1284B, stderr 0B                       567.492ms
step   progress-inspect-sandbox                                                     571.230ms
total  3 external commands 2290.552ms, sbxm 113.561ms                              2404.113ms
```

| Line | Meaning |
| --- | --- |
| `run` | An external command starts; its command line follows |
| `done` | It ended, with its exit status and the bytes it wrote; the elapsed time is on the right |
| `failed` | It returned no result, such as a timeout; the error ID follows |
| `inside` | One of the checks batched into a single `sbx exec` inside the sandbox, timed by the sandbox shell |
| `step` | A progress step ended; its message ID follows |
| `total` | The wall time, split into time inside external commands and time sbxm itself used |

The trace shows command lines but never what sbxm sent on standard input or what a command printed; those appear only as byte counts. Standard output stays the result alone.
