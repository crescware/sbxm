# sbxm

`sbxm` gives each GitHub project its own Docker Sandbox and a predictable set
of Git worktrees. It handles the host clone, sandbox image, repository setup,
day-to-day connections, diagnostics, rebuilds, and teardown.

The sandbox receives the Git identity and configuration files you choose, but
not your host project directory, Docker socket, or SSH agent. GitHub credentials
are supplied through the Docker Sandboxes secret proxy instead of being copied
into the sandbox.

日本語版: [docs/README.ja.md](docs/README.ja.md)

## Requirements

- macOS 14 or later on Apple silicon
- Docker Desktop with a running Docker Engine
- **[Docker Sandboxes CLI 0.37.0 or later](https://docs.docker.com/ai/sandboxes/get-started/)**
- Git and SSH
- A GitHub personal access token for each repository you want to manage

Run `sbxm status --global` after initialization to check these requirements and
the Docker Sandboxes environment.

## Installation

> [!WARNING]
> Installation is not available yet. This section describes the planned
> Homebrew interface and is included as a draft.

```sh
brew install crescware/tap/sbxm
```

## Quick start

### 1. Initialize sbxm

```sh
sbxm init
```

The interactive setup creates `~/.sbxm/config.toml` and asks for:

- the directory where host-side project clones should live;
- the Git name and email to use inside sandboxes.

It chooses the display language from your system locale and lets you confirm
the choice when needed.

For non-interactive setup, provide all three configuration values:

```sh
sbxm init \
  --lang en \
  --base-path "$HOME/Projects" \
  --git-user-name "Your Name" \
  --git-user-email "you@example.com"
```

Then verify the host:

```sh
sbxm status --global
```

### 2. Register a project

Projects are identified as `owner/repository`:

```sh
sbxm add owner/repository
```

This registers the project, creates its host clone and Dockerfile, and prints
the sandbox name and exact next commands. It does not build the sandbox yet.

By default, the sandbox gets one worktree on the repository's default branch.
For several independent worktrees, choose a starting branch and detached mode:

```sh
sbxm add owner/repository --detach main --worktrees 3
```

Detached worktrees are useful when several agents or tasks need isolated
working directories. The supported count is 1–32.

### 3. Register the GitHub credential

Create a personal access token that can read and write the repository:

- a fine-grained token needs **Contents: read and write** and
  **Metadata: read**;
- a classic token needs the `repo` scope.

`sbxm add` prints a project-specific `sbx secret set-custom` command. Run that
command with your token before preparing the project. The command resembles:

```sh
sbx secret set-custom <sandbox> \
  --host github.com \
  --host '**.github.com' \
  --host '**.githubusercontent.com' \
  --host ghcr.io \
  --env GH_TOKEN \
  --value <token>
```

The secret proxy keeps the real token outside the sandbox. The sandbox sees a
placeholder, which the proxy replaces only for requests to the registered
hosts.

### 4. Build and enter the sandbox

```sh
sbxm prepare owner/repository
sbxm open owner/repository
```

`prepare` builds the project image, creates the sandbox, clones the repository
inside it, and creates the managed worktrees. `open` starts a stopped sandbox
when necessary and connects over SSH.

Inside the sandbox, worktrees are located at:

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
...
```

## Daily use

```sh
# See every managed project and its sandbox state
sbxm ls

# Inspect one project without changing it
sbxm status owner/repository

# Connect to a project
sbxm open owner/repository

# Stop one or more projects without deleting them
sbxm stop owner/repository
sbxm stop owner/repository another/project
```

When run in an interactive terminal, `open`, `stop`, and `destroy` can prompt
you to select a project if the project argument is omitted.

## Customize a project

### Edit the sandbox image

`sbxm add` creates a Dockerfile in the project's host-side directory. Edit that
file to add tools or system dependencies, then apply it:

```sh
sbxm rebuild owner/repository
```

Rebuilding recreates the sandbox. To protect work, sbxm refuses a normal
rebuild when worktrees contain dirty files, unpushed commits, or unmanaged
worktrees.

### Add managed worktrees

A built project can gain more managed worktrees without a rebuild:

```sh
sbxm apply owner/repository --worktrees 4
```

The count can only increase. For a project registered in the default attached
mode, the first worktree stays on its tracking branch and additional worktrees
are detached.

### Place configuration files

Declare host files in `~/.sbxm/config.toml`:

```toml
[[files]]
source = "/Users/you/.config/example/config.toml"
destination = ".config/example/config.toml"

[[files]]
source = "/Users/you/.config/another-tool/settings.toml"
destination = ".config/another-tool/settings.toml"
```

The destination is relative to the sandbox user's home directory. Declarations
are placed during `prepare`; apply later changes explicitly:

```sh
sbxm apply owner/repository --files
```

`--files` overwrites the declared destinations. Keep tokens, private keys, and
other credentials out of these files; use Docker Sandboxes secrets for those.

Both apply scopes may be requested together:

```sh
sbxm apply owner/repository --files --worktrees 4
```

## Tear down a project

```sh
sbxm destroy owner/repository
```

Before deleting anything, sbxm shows what will be removed and what will remain.
Normal teardown checks for dirty worktrees, unpushed commits, and active
sessions. In an interactive terminal, it then asks you to type the sandbox
name.

The sandbox and sbxm's project metadata are deleted. The host clone, project
Dockerfile, built images, loaded templates, and registered secrets are kept, so
the project can be registered again later.

If you intentionally need to bypass data-protection and active-session checks:

```sh
sbxm destroy --force owner/repository
```

Use `--force` only when you have independently confirmed that nothing inside
the sandbox needs to be preserved.

## Command overview

| Command | Purpose |
|---|---|
| `sbxm init` | Create the global configuration |
| `sbxm add owner/repository` | Register a GitHub project and create its host artifacts |
| `sbxm prepare owner/repository` | Build and provision the project's sandbox |
| `sbxm open [owner/repository]` | Start the sandbox if needed and connect over SSH |
| `sbxm stop [owner/repository ...]` | Stop one or more sandboxes |
| `sbxm ls` | List managed projects and sandbox states |
| `sbxm status --global` | Diagnose the host and Docker Sandboxes environment |
| `sbxm status owner/repository` | Diagnose a project without changing it |
| `sbxm apply owner/repository ...` | Apply declared files or add managed worktrees |
| `sbxm rebuild owner/repository` | Recreate a sandbox from its edited Dockerfile |
| `sbxm destroy [owner/repository]` | Delete a sandbox and stop managing the project |

Use `sbxm --help` or `sbxm <command> --help` for the complete CLI reference.
