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

## Declared files

The `DECLARED FILES` section of a project's status compares every current
declaration with what sbxm placed at that destination last, once on the host and
once in the sandbox.

| Value | Column | Meaning | What to do |
| --- | --- | --- | --- |
| `unchanged` | both | The same content sbxm placed last. In a sandbox without a record, the same as the host file. | Nothing. |
| `updated` | HOST | The host file changed after sbxm placed it. | Run `sbxm apply <project-id> --files`. |
| `unplaced` | HOST | sbxm has not placed this declaration in this project yet. | Run `sbxm apply <project-id> --files`; a project without a sandbox gets it from its first build. |
| `unreadable` | HOST | The host file cannot be placed as it is. The diagnostic below the table says why. | Fix the file or remove the declaration with `sbxm files rm`. |
| `modified` | SANDBOX | The sandbox copy was changed after sbxm placed it. `apply --files` leaves it alone. | Bring the change back with `sbxm files pull`, or save what you need from it before replacing it with `--force`. |
| `unrecorded` | SANDBOX | The sandbox copy differs from the host file, and sbxm has no record of placing it. | The same as `modified`. |
| `missing` | SANDBOX | The sandbox has no file at the destination. | Run `sbxm apply <project-id> --files`. |

Status shows the `apply --files` command only while it can be run as it is: the sandbox is running and was looked into, and no other next step is shown. A next step such as `open` or `repair` places the declared files itself.

A stopped sandbox is reported as `not-observed-stopped` and is not started to be read.

## The next command

`sbxm status <project-id>` ends with at most one command, chosen from the same observation
that `sbxm repair` uses, so the two never disagree.

| What was observed | Next command |
| --- | --- |
| The first provisioning started and did not finish, or part of its result is missing. | `sbxm open <project-id>` — preparation resumes and then connects. |
| A generation change started and did not finish. | `sbxm rebuild <project-id>` — the same generation change is completed. |
| The project is usable and the Dockerfile differs from the applied generation. | `sbxm rebuild <project-id>` — the current Dockerfile is applied as a new generation. |
| Recovery and a generation change are both relevant. | `sbxm open <project-id>` first. Run status again afterwards to see whether a rebuild is still needed. |
| Nothing can be proven safe: an identity mismatch, an unobservable artifact, or a stopped sandbox that has no first-provisioning intent. | None. Status reports the observation instead of naming a command it cannot prove is safe. |
