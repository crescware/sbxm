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

The destination is relative to the sandbox user’s home directory. Declarations are placed while the sandbox is first built.

## Apply a later change

```sh
sbxm apply <project-id> --files
```

`--files` replaces a destination only while it still holds what sbxm placed there last. sbxm records the content of every file it places, per project. If a file was edited inside the sandbox, or sbxm has no record of placing it, `apply` refuses before placing anything and names every such file instead of deciding which version should win.

Save what you need from the sandbox copy, then replace it explicitly:

```sh
sbxm apply <project-id> --files --force
```

Editing a declared file inside the sandbox is not treated as damage: `open` and `repair` neither refuse the project nor put the declared file back over the edit.

## Apply to every project

```sh
sbxm apply --files --all
```

Each registered project is locked and handled on its own under the same rules. One project that cannot be applied does not stop the others. Stopped sandboxes are not started, and a project without a sandbox gets the declarations from its first build; both appear in the result table. The exit status is `1` when any project could not be applied.

Both apply scopes may be requested together:

```sh
sbxm apply <project-id> --files --worktrees 4
```

## Keep credentials out

Do not put tokens, private keys, or other credentials in declared files. Use Docker Sandboxes custom secrets so the real credential stays outside the sandbox. See [Create your first sandbox](../../getting-started/quickstart/) for the GitHub token flow.
