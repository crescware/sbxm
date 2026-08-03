---
title: Place configuration files
description: Declare safe host files and apply them inside sbxm sandboxes.
---

Declare host files in `~/.sbxm/config.yaml`:

```yaml
version: 1

files:
  - source: /Users/you/.gitconfig
    destination: .gitconfig

  - source: /Users/you/.config/another-tool/settings.yaml
    destination: .config/another-tool/settings.yaml
```

The destination is relative to the sandbox user’s home directory. Declarations are placed during `prepare`.

## Apply a later change

```sh
sbxm apply <project-id> --files
```

`--files` re-places the declared destinations and overwrites what is there. If a destination contains different content, sbxm refuses the conflict instead of deciding which file should win.

Both apply scopes may be requested together:

```sh
sbxm apply <project-id> --files --worktrees 4
```

## Keep credentials out

Do not put tokens, private keys, or other credentials in declared files. Use Docker Sandboxes custom secrets so the real credential stays outside the sandbox. See [Create your first sandbox](/getting-started/quickstart/) for the GitHub token flow.
