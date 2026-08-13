---
title: Status values
description: Understand the state values reported by sbxm status and sbxm ls.
---

`sbxm status` and `sbxm ls` use stable, untranslated values so they can be recognized
across locales. The values below describe the observation result and the action it
suggests.

| Value | Meaning | What to do |
| --- | --- | --- |
| `ready` | The observed state is available and matches the project declaration. | Continue with the normal workflow. |
| `missing` | The expected item was observed to be absent. | Create or restore the item, then run the command again. |
| `mismatch` | The observed state does not match the project declaration. | Inspect the diagnostic and fix the project declaration or the observed artifact. |
| `not-observed` | sbxm could not observe the state, so it cannot say whether the item is present or matches. | Read the diagnostic below the table and fix the environment that prevented observation. |

`not-observed` is different from `missing`: absence is an observation, while
`not-observed` means that the check itself could not produce an answer.
