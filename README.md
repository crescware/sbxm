# sbxm

A Rust CLI that sets up, connects to, diagnoses and tears down a Docker Sandbox per project.

日本語版: [docs/README.ja.md](docs/README.ja.md)

`sbxm` is a wrapper and orchestrator around Docker Sandboxes, not a system that owns or tracks
who produced a given artifact. Whether metadata, a sandbox, a workspace, an image, a Git
repository or a worktree was created by `sbxm` never decides whether it can be used. State
created by hand or by another tool is accepted as the same state as long as it satisfies the
validation rules.

- Direction: [plans/docker-sandbox-automation-mvp.md](plans/docker-sandbox-automation-mvp.md)
- Per-phase specifications: [plans/specs/](plans/specs/)

## Implementation status

The MVP is implemented in four phases. Phase 1 is done.

| Phase | Scope | State |
|---|---|---|
| 1 | Shared foundation, `init`, `status --global` | Implemented |
| 2 | `add`, `sync-files` | Not started |
| 3 | `open`, `stop`, `ls`, `status <project>` | Not started |
| 4 | `rebuild`, `destroy`, end-to-end validation | Not started |

All nine commands are registered with the parser, so help and command-specific argument
validation work for every command. A command that is not implemented yet exits with
`not-implemented` after its arguments have been validated.

## Target environment

- macOS Sonoma 14 or later on an Apple silicon Mac
- Docker Desktop and Docker Sandboxes CLI 0.37.0 or later
- GitHub repositories and the GitHub CLI
- An editor with Remote SSH support

The Docker Sandboxes CLI is in Early Access, so "0.37.0 or later always works" is not assumed.
The external commands and structured output that sbxm relies on are pinned by fixtures
collected from the target version.

## Usage

```sh
sbxm [--lang <ja|en>] init
sbxm [--lang <ja|en>] init --base-path <PATH> --git-user-name <NAME> --git-user-email <EMAIL>
sbxm [--lang <ja|en>] status --global
```

`--lang` is accepted before or after the subcommand. Pass `--lang en` when the output is
consumed by a script or a pipe: the Japanese mode's stdout is not a machine-readable contract.

Only three exit codes are used.

| Code | Meaning |
|---:|---|
| `0` | Success, or a no-op that the specification defines as success |
| `1` | Invalid arguments, ordinary cancellation, unmet prerequisites, invalid configuration or state, external command failure, or a refusal on safety grounds |
| `130` | Interactive cancellation with Ctrl-C or Esc |

Failures are not classified by exit code. Each one carries a stable English error ID that is
never translated, together with an explanation in the selected language.

## Build and test

```sh
cargo build
cargo test
```

The published CLI contract — command names, option names, value names, arity and order — is
recorded in `tests/snapshots/cli-surface.txt`. It carries no translated text, so it does not
change when a language is added. Update it only after reviewing an intentional change to the
contract.

```sh
SBXM_UPDATE_SNAPSHOTS=1 cargo test --bin sbxm
```

## Display languages

Every user-facing string comes from an FTL resource in [locales/](locales/). Adding a language
means adding one resource and one row to the locale definition table in `src/i18n.rs`; nothing
else is edited. See [locales/README.md](locales/README.md) for the conventions.

## Docker Sandboxes CLI fixtures have not been collected

`validated_cli_versions` in `compatibility.toml` is empty. In that state sbxm treats every
detected Docker Sandboxes CLI version as one whose output it cannot interpret, and stops any
check that depends on `sbx` output with `sbx-fixtures-not-collected`. Until the fixtures are
collected, the Docker Sandboxes, Login, Network policy, Remote SSH, Daemon and Session
inspection rows of `sbxm status --global` report `error`.

This is not an unimplemented path. It applies the rule "never report a state that was inferred
rather than observed" to the case where no fixture exists yet. The collection procedure is in
[tests/fixtures/sbx/README.md](tests/fixtures/sbx/README.md).

Collection requires the target Mac (macOS 14 or later on Apple silicon, Docker Desktop and the
Docker Sandboxes CLI), which the environment that produced the Phase 1 pull request did not
have.

## Configuration

`~/.sbxm/config.toml`, mode `0600`, inside a `0700` `~/.sbxm`.

```toml
version = 1
language = "ja"
base_path = "/Users/example/Projects"

[git]
user_name = "Example User"
user_email = "user@example.com"

[[files]]
source = "/Users/example/.config/example/config.toml"
destination = ".config/example/config.toml"
```

- No token, secret or runtime state is stored.
- `files` declares regular host files to place under the `agent` home inside the sandbox. Do
  not use it for credentials, tokens or private keys; use the Docker Sandboxes secret feature
  for those.
- sbxm never repairs or overwrites an invalid configuration. Edit it directly and it is
  validated again on the next run.
