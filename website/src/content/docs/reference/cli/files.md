---
title: '`sbxm files`'
description: Declare host files to place in every sandbox, list them, or remove a declaration.
---

```text
sbxm files add <path> [--dest <destination>]
sbxm files ls
sbxm files rm <destination>
```

`files` edits the `files` list in `~/.sbxm/config.yaml`. It changes declarations only; files are placed in sandboxes by the first build and by [`sbxm apply --files`](../apply/).

## `add`

Declares one host file. A relative path is resolved from the current directory. The destination defaults to the file's path relative to your home directory, so `~/.claude/CLAUDE.md` is placed at `.claude/CLAUDE.md` in the sandbox home. Use `--dest` for a file outside your home directory or to place it elsewhere.

The file is checked before the declaration is saved: it must be a regular file (not a directory or a symbolic link) within the size limit, and the destination must stay inside the sandbox home. A name that often holds credentials, such as `.env`, `credentials`, `id_rsa`, or a name containing `token`, is saved with a warning; keep credentials in Docker Sandboxes secrets instead.

A destination that is already declared with another source is refused. Declaring the same file again changes nothing.

After a new declaration is saved, an interactive terminal asks whether to place the declared files in every registered project now, which runs the same steps as `sbxm apply --files --all`. Otherwise the command shows that command to run later.

The existing configuration keeps its comments and formatting. The new entry follows the style of the existing entries. If sbxm cannot confirm that the rest of the file stays as written, it refuses and shows the entry to add by hand.

## `ls`

Lists the declared files with their destinations.

## `rm`

Removes the declaration whose destination is `<destination>`. Copies already placed in sandboxes are left as they are.
