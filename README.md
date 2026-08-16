# sbxm

<p align="center">
  <a href="https://crescware.github.io/sbxm/">
    <img src="website/src/assets/sbxm-logo-color.svg" alt="sbxm" width="180">
  </a>
</p>

<p align="center">
  <a href="https://crescware.github.io/sbxm/">Visit the official sbxm website</a>
</p>

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

Run `sbxm status --global` to check these requirements and the Docker Sandboxes
environment.

## Installation

```sh
brew install crescware/tap/sbxm
```

## Quick start

### 1. Verify the host

Check that the host has what sbxm needs:

```sh
sbxm status --global
```

sbxm reads the Git name and email it will use inside sandboxes from your own
account, so declare them once if you have not already:

```sh
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

### 2. Register a project

`cd` to the directory that should hold the project, then pass the clone URL
GitHub shows for the repository, unchanged:

```sh
cd ~/Projects
sbxm add git@github.com:<owner>/<repository>.git
```

```sh
sbxm add https://github.com/<owner>/<repository>.git
```

These two forms are the only ones `sbxm add` accepts. The host clone uses the
transport you pass.

sbxm creates `<repository>.project/` in the directory you ran it from — you do
not make a directory per project or follow a naming rule. The first
interactive `add` also asks once which language sbxm should speak, and
remembers the answer in `~/.sbxm/config.yaml`.

The same first `add` asks which name and email the project's commits are made
under. Your host `git config --global` values are offered as the starting text,
so pressing Enter twice accepts them, and typing over them chooses something
else. The answer is saved in `~/.sbxm/config.yaml` and later runs never ask
again. sbxm never reads your host Git settings as the answer on its own.

Each project also keeps its own copy of the identity, written when it is
registered. Changing the saved default afterwards leaves projects you already
registered under the name they were registered with.

To use a different identity for one project, or to register without a terminal
to answer on, declare both halves:

```sh
sbxm add git@github.com:<owner>/<repository>.git \
  --git-user-name '<name>' --git-user-email '<email>'
```

Declaring them applies to that run only and does not change the saved default,
the same way `--lang` does not change the saved language. Passing only one of
the two is refused before anything is read or created. A run with no terminal,
no saved default, and no declaration stops rather than guessing.

This registers the project, creates its host clone and Dockerfile, and prints
the sandbox name and exact next commands. It does not build the sandbox yet.

By default, the sandbox gets one worktree on the repository's default branch.
For several independent worktrees, choose a starting branch and detached mode:

```sh
sbxm add git@github.com:<owner>/<repository>.git --detach main --worktrees 3
```

Detached worktrees are useful when several agents or tasks need isolated
working directories. The supported count is 1–32. `--worktrees` may be
shortened to `-t`.

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
sbxm prepare <project-id>
sbxm open <project-id>
```

`prepare` builds the project image, creates the sandbox, clones the repository
inside it, and creates the managed worktrees. `open` starts a stopped sandbox
when necessary and connects over SSH.

The session starts in `/home/agent/work/<repository>`. To start in a managed
worktree, use its zero-based index, for example `sbxm open <project-id> -i 0`.

When the project ID is omitted in an interactive terminal, sbxm shows one
prompt. Use the up and down cursor keys to choose a project, the left and right
cursor keys to adjust its zero-based managed worktree index, and press Enter
once to confirm both. So that it appears immediately, the prompt opens without
reading project metadata. Until that project's result arrives, the index line
reads `(calculating)` rather than naming a range sbxm cannot yet know; the index
still moves in the meantime. Metadata is calculated in the background, and when
the result arrives the prompt shows that project's own range and holds the index
within it. Confirmation rechecks the value under the project lock, and sbxm
warns before connecting if the confirmed index had to be brought down.

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
sbxm status <project-id>

# Connect to a project
sbxm open <project-id>

# Stop one or more projects without deleting them
sbxm stop <project-id>
sbxm stop <project-id> ...
```

The `STATE` column answers whether `sbxm open` can proceed directly: `stopped`
means opening starts the sandbox, while `open-blocked` means a startup
prerequisite needs recovery first. Use `sbxm status <project-id>` for the
reason.

When run in an interactive terminal, `prepare`, `apply`, `rebuild`, `open`,
`stop`, `destroy`, and `status` can prompt you to select a target if the
project argument is omitted. For `status`, the first choice is `global`,
followed by registered project IDs.
In a non-interactive terminal, provide an explicit project argument for these
commands; `status` accepts either a project ID or `--global`. Normal `rebuild`
and `destroy` still refuse in a non-interactive terminal because their protected
flows require an interactive, exact sandbox-name confirmation. `destroy --force`
is the only non-interactive bypass for destroy; it skips those checks
and does not make the discarded data recoverable.

## Customize a project

### Edit the sandbox image

`sbxm add` creates a Dockerfile in the project's host-side directory. Edit that
file to add tools or system dependencies, then apply it:

```sh
sbxm rebuild <project-id>
```

Rebuilding recreates the sandbox from the Dockerfile whether or not it
changed. The old sandbox's writable layer is lost. To protect work, sbxm
refuses a normal rebuild when worktrees contain dirty files, unpublished
commits, in-progress Git operations, or unmanaged worktrees.

The protection pass also checks repository-level state that a clean worktree
does not show by itself: local branches kept out of the checkout, tags, notes,
stash entries, extra remotes, and reflog-only commits. Save or resolve any
reported Layer A blocker before retrying. `status` keeps the worktree's
`STATE` and its origin recovery evidence in a separate `REMOTE` column.
Normal rebuild always requires an interactive plan and exact sandbox-name
confirmation; it refuses rather than silently skipping that confirmation in a
non-interactive terminal.

### Choose the sandbox root size

Docker Sandboxes reads `DOCKER_SANDBOXES_ROOT_SIZE` from the environment of the
process that creates a sandbox. sbxm does not interpret or rewrite this
variable; it passes through whatever is set when it runs `sbx create`:

```sh
# First creation
DOCKER_SANDBOXES_ROOT_SIZE=40g sbxm prepare <project-id>

# Re-creation, after the sandbox already exists
DOCKER_SANDBOXES_ROOT_SIZE=40g sbxm rebuild <project-id>
```

A few things this does *not* do:

- The variable only takes effect on the sandbox being created. It is not an
  in-place resize of an existing sandbox's filesystem; changing the size
  requires recreating the sandbox, so `rebuild` still goes through the same
  data-protection checks as any other rebuild.
- The requested size is not reserved up front. It raises the ceiling each
  sandbox can grow into; actual usage across all of a host's sandboxes still
  adds up against the host's real disk.
- Built images and loaded templates live outside each sandbox's root
  filesystem and consume host space of their own, independent of this
  setting. The archive sbxm exports in between is a transient file removed
  once the load finishes; it does not accumulate.

Check the host has headroom for the size you request before running either
command.

### Understand what fills the sandbox's disk

`sbxm status <project-id>` shows a DISK section with the sandbox's current
free space, usable ceiling, and capacity. A few facts explain what that number
reflects:

- `/home`, `/tmp`, and everything else inside the sandbox share one root
  filesystem — the same one sized by `DOCKER_SANDBOXES_ROOT_SIZE` above. There
  is no separate volume for build output or temporary files.
- `/tmp` is not cleared by stopping and reopening a sandbox (`sbxm open`).
  There is no init system running periodic cleanup inside it; files placed
  there persist until the sandbox itself is destroyed or rebuilt.
- Deleting a file inside the sandbox reclaims that space immediately — the
  root filesystem is a normal writable layer, not a snapshot that only frees
  up on recreation.
- Each managed worktree builds independently, so `--worktrees N` multiplies
  build artifacts (for example, Rust's `target/`) by however many worktrees
  are configured.
- Projects that support a shared build cache directory (for example Rust's
  `CARGO_TARGET_DIR`) can point every worktree at the same directory via a
  declared file (see "Place configuration files" below) to avoid that
  multiplication. Sharing one directory serializes what would otherwise be
  concurrent builds across worktrees, so treat it as an explicit trade-off,
  not a default.

When disk recovery is needed, use this order:

1. Delete unnecessary files inside the sandbox; the space returns immediately.
2. Inspect what a rebuild would discard. If recreating the writable layer is
   necessary, run the normal protected `rebuild` flow and review its plan.
3. Resolve every Layer A blocker shown by the protection checks — including
   unpublished commits, dirty or untracked work, Git operations, active
   sessions, and repository-level refs — before retrying.

### Add managed worktrees

A built project can gain more managed worktrees without a rebuild:

```sh
sbxm apply <project-id> --worktrees 4
```

The count can only increase. For a project registered in the default attached
mode, the first worktree stays on its tracking branch and additional worktrees
are detached. Here too, `--worktrees` may be shortened to `-t`.

### Place configuration files

Declare host files in `~/.sbxm/config.yaml`:

```yaml
version: 1

files:
  - source: /Users/you/.gitconfig
    destination: .gitconfig

  - source: /Users/you/.config/another-tool/settings.yaml
    destination: .config/another-tool/settings.yaml
```

The destination is relative to the sandbox user's home directory. Declarations
are placed during `prepare`; apply later changes explicitly:

```sh
sbxm apply <project-id> --files
```

`--files` overwrites the declared destinations. Keep tokens, private keys, and
other credentials out of these files; use Docker Sandboxes secrets for those.

Both apply scopes may be requested together:

```sh
sbxm apply <project-id> --files --worktrees 4
```

## Tear down a project

```sh
sbxm destroy <project-id>
```

Before deleting anything, sbxm shows what will be removed and what will remain.
Normal teardown checks for dirty worktrees, unpublished commits, repository-level
refs, and active sbxm sessions. In an interactive terminal, it then asks you to
type the sandbox name. Removing the sandbox itself also respects Docker
Sandboxes' own runtime check for anything still attached to it (a session sbxm
did not start) — sbxm answers that confirmation internally, so you are not
asked twice. A normal destroy in a non-interactive terminal refuses rather than
skipping the exact-name confirmation.

The sandbox, sbxm's project metadata, and the `GH_TOKEN` custom secret
registered for that sandbox are deleted. A registration left behind would make
the next `sbx secret set-custom` for the same project fail as a duplicate, and
it would keep a token for a sandbox that no longer exists. The host clone,
project Dockerfile, built images, loaded templates, and every secret registered
for anything else are kept, so the project can be registered again later with a
token registered anew. Because those artifacts stay behind and sbxm never
adopts a directory it did not register, registering the project again in the
same place means moving them aside first.

If you intentionally need to bypass data-protection, active-session, and
runtime in-use checks, and the confirmation prompt:

```sh
sbxm destroy --force <project-id>
```

Use `--force` only when you have independently confirmed that nothing inside
the sandbox needs to be preserved.

## Where sbxm keeps things

A project lives entirely in the directory you registered it from:

```text
<parent>/<repository>.project/
├── <repository>/       # the host clone
└── .sbxm/              # metadata, Dockerfile, lock, cache
```

Under `~/.sbxm`, sbxm keeps `registry.yaml` — the index of registered projects
and where each one lives — and, once you have chosen a display language or an
identity, or declared files, `config.yaml`. The registry is the only thing that knows where
a project is, so moving a project directory makes `ls` report it as `missing`
rather than sbxm guessing at the new location.

## Command overview

| Command | Purpose |
|---|---|
| `sbxm add <github-clone-url>` | Add a GitHub repository to sbxm and clone it onto this host |
| `sbxm prepare [<project-id>]` | Prepare a registered project by building and provisioning its sandbox |
| `sbxm open [<project-id>] [--index N]` | Open an SSH session to a project sandbox, starting it if needed; `N` selects a zero-based managed worktree |
| `sbxm stop [<project-id> ...]` | Stop one or more project sandboxes without deleting them |
| `sbxm ls` | List managed projects and unmanaged sandboxes with their states |
| `sbxm status` | Select and show the host or a project's status interactively; `global` is first |
| `sbxm status --global` | Show the host environment status without changing it |
| `sbxm status <project-id>` | Show a project's status without changing it |
| `sbxm apply [<project-id>] ...` | Apply declared files or add managed worktrees |
| `sbxm rebuild [<project-id>]` | Rebuild a project's sandbox from its Dockerfile; the old writable layer is lost |
| `sbxm destroy [<project-id>]` | Destroy a project's sandbox and stop managing the project, keeping its host clone and Dockerfile |

Use `sbxm --help` or `sbxm <command> --help` for the complete CLI reference.

## Output

sbxm writes results to standard output and progress, prompts, warnings and
errors to standard error, so a redirected result stays free of anything but the
result.

Color is decided per stream. When only standard output is piped, the result is
plain text while the diagnostics left on the terminal keep their color. Colors
come from the ANSI palette your terminal theme defines rather than from fixed
values, so they follow the contrast you already chose.

| Setting | Effect |
|---|---|
| `--color=auto` | Color a stream only when it is a terminal (the default) |
| `--color=always` | Color even a redirected stream |
| `--color=never` | Never color anything |
| `NO_COLOR` | Turns color off whatever its value, including empty |
| `CLICOLOR_FORCE` | Turns color on unless it is `0` |
| `TERM=dumb` | Turns color off and falls back to ASCII markers |

An explicit `--color` wins over every environment variable. Removing color
never removes information: markers, labels and blank lines carry the same
meaning on their own.
