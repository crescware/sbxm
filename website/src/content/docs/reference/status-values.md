---
title: Status values
description: Understand the state values reported by sbxm status and sbxm ls.
---

`sbxm status` uses stable, untranslated values so they can be recognized across locales.
The values below describe an observation result and the action it suggests. `sbxm ls`
uses a smaller user-facing project-state vocabulary; its `open-blocked` state points to
the detailed observations in `sbxm status <project-id>`.

| Value | Meaning | What to do |
| --- | --- | --- |
| `ready` | The observed state is available and matches the project declaration. | Continue with the normal workflow. |
| `missing` | The expected item was observed to be absent. | Create or restore the item, then run the command again. |
| `mismatch` | The observed state does not match the project declaration. | Inspect the diagnostic and fix the project declaration or the observed artifact. |
| `not-observed` | sbxm could not observe the state, so it cannot say whether the item is present or matches. | Read the diagnostic below the table and fix the environment that prevented observation. |
| `not-applicable` | There is nothing to look into yet, so the item has no state to report. | Nothing. The item becomes meaningful once the artifact it describes exists. |

`not-observed` is different from `missing`: absence is an observation, while
`not-observed` means that the check itself could not produce an answer.

## The next command

`sbxm status <project-id>` ends with at most one command, chosen from the same observation
that `sbxm repair` uses, so the two never disagree.

| What was observed | Next command |
| --- | --- |
| The first provisioning started and did not finish, or part of its result is missing. | `sbxm repair <project-id>` — recovery goes back to the generation the project already fixed. |
| A generation change started and did not finish. | `sbxm rebuild <project-id>` — the same generation change is completed. |
| The project is usable and the Dockerfile differs from the applied generation. | `sbxm rebuild <project-id>` — the current Dockerfile is applied as a new generation. |
| Recovery and a generation change are both relevant. | `sbxm repair <project-id>` only. Run status again afterwards to see whether a rebuild is still needed. |
| Nothing can be proven safe: an identity mismatch, an unobservable artifact, or a sandbox that is not running. | None. Status reports the observation instead of naming a command it cannot prove is safe. |
