# sbxm

A Rust CLI that sets up, connects to, diagnoses and tears down a Docker Sandbox per project.

日本語版: [docs/README.ja.md](docs/README.ja.md)

## Manual verification

The automated tests replace every external command with a fake, so they say
nothing about the Docker Sandboxes CLI on a real machine. This section is the
record of running sbxm against it. Every case is listed with the command, the
expected exit code and the expected result; fill in the observed result and the
CLI version when you run it.

Run it on the target platform only: macOS 14 or later on Apple silicon, with
Docker Desktop and the Docker Sandboxes CLI 0.37.0 or later.

### Before you start

- Use a private test repository. Never make a real project the first subject.
- Use a throwaway `HOME` so the run cannot touch your own configuration:
  `env HOME="$(mktemp -d)" sbxm ...`
- Record the exact CLI version: `sbx version`, `docker version`.

### The GitHub token

`prepare` clones the repository from inside the sandbox, which has no SSH
agent, so it authenticates with a token stored as a Docker Sandboxes secret.
`add` prints the sandbox name and the command that registers it; the token is
registered between `add` and `prepare`.

The token is registered as a custom secret, not as the `github` service secret:

```
sbx secret set-custom <sandbox> --host github.com --env GH_TOKEN --value <token>
```

A custom secret shows the sandbox a placeholder and leaves the real token with
the proxy, which substitutes it into the request headers that go to github.com.
The token never enters the sandbox, and the token type does not matter. The
`github` service secret was not usable here: on real hardware it authenticated a
fine-grained token and left a classic one unauthenticated.

A custom secret binds to a sandbox when the sandbox is created, so registering
it afterwards does not reach a sandbox that already exists. `prepare` asks for
the secret before it creates anything, and looks inside the sandbox afterwards
to confirm the placeholder arrived.

Issue a token that can read and write that one repository.

| Token | Setting |
|---|---|
| Fine-grained | Contents read and write, Metadata read |
| Fine-grained, optional | Pull requests, Issues, Actions, only if the work needs them |
| Classic | The `repo` scope |

`add` prints these requirements too, so they do not have to be remembered.

### Redaction

Before recording anything, remove:

- tokens and secret values of any kind
- the macOS user name inside paths, replaced by `<user>`
- SSH public keys and agent socket paths
- repository names, if the test repository is not public

### Cases

| # | Command | Expected exit | Expected result |
|---:|---|---:|---|
| 1 | `sbxm init` in a fresh HOME | 0 | `~/.sbxm/config.toml` created with mode 0600 |
| 2 | `sbxm --lang ja init`, `sbxm --lang en init` | 0 | help and output follow the chosen language |
| 3 | `sbxm add <owner>/<repo>` | 0 | registers the project, clones it onto the host, and names the sandbox and the token command |
| 4 | register the secret, then `sbxm prepare <owner>/<repo>` | 0 | builds the sandbox and the repository in one run |
| 5 | `sbxm prepare <owner>/<repo>` without the secret | 1 | stops at `github-secret-missing` with the command that registers it, before it builds an image or creates a sandbox |
| 5a | register the secret after the sandbox exists, then `sbxm prepare` | 1 | stops at `sandbox-secret-not-applied` and names the `sbx rm` that lets it be built anew |
| 5b | inside the sandbox, `git ls-remote origin` in the bare repository | 0 | git authenticates with the placeholder and never asks for a username |
| 6 | `sbxm add <owner>/<repo2> --worktrees 3 --detach develop` then `sbxm prepare` | 0 | three worktrees on the same commit of `origin/develop` |
| 7 | create an extra worktree inside the sandbox by hand | - | it is the unmanaged case for the later checks |
| 8 | inspect the sandbox workspace | - | no project path and no user home is visible inside |
| 9 | `ssh-add -L` and `docker info` inside the sandbox | non-zero | no agent keys, no host Docker socket |
| 10 | run Codex, Claude Code and `gh auth status` inside | 0 | each reaches the network it needs |
| 11 | `sbxm open <owner>/<repo>` from stopped and from running | 0 | both connect; the stopped one is started first |
| 13 | `sbxm stop <a> <b>` and then again | 0 | first stops both, second is a no-op |
| 14 | `sbxm ls` | 0 | running, stopped and not-created appear, unmanaged sandboxes separately |
| 15 | `sbxm status <owner>/<repo>` | 0 or 1 | managed and unmanaged worktrees, dirty state and SSH agent are reported |
| 16 | `sbxm sync-files <owner>/<repo>` | 0 | only the declared files change |
| 17 | `sbxm rebuild <owner>/<repo>` with no Dockerfile change | 0 | reports that nothing was applied |
| 18 | break the Dockerfile, then `sbxm rebuild` | 1 | the build fails and the existing sandbox still runs |
| 19 | `sbxm rebuild` with a dirty tree or unpushed commits | 1 | `unsaved-work`, naming what would be lost |
| 20 | `sbxm rebuild` with the unmanaged worktree from case 7 | 1 | `unmanaged-worktree-present`, naming the worktree and how to remove it |
| 21 | `sbxm rebuild` on a stopped sandbox | 0 | starts it to read its saved state, then rebuilds |
| 22 | `sbxm rebuild` on a clean, managed-only sandbox | 0 | the new generation is applied |
| 23 | interrupt case 22 right after the sandbox is removed, then rerun | 0 | continues from the recorded generation |
| 24 | `sbxm status <owner>/<repo>` after case 22 | 0 | the new Dockerfile hash, worktrees, files and Git identity are in place |
| 25 | `sbxm destroy` with a dirty managed worktree | 1 | `unsaved-work` |
| 26 | `sbxm destroy` with a dirty unmanaged worktree | 1 | `unsaved-work` |
| 27 | `sbxm destroy` with unpushed commits | 1 | `unsaved-work` |
| 28 | `sbxm destroy` on a clean project, typing the sandbox name | 0 | deleted after the typed confirmation |
| 29 | `sbxm destroy --force` on a running sandbox with unsaved work | 0 | deleted, with the skipped checks stated |
| 30 | `sbxm destroy --force` on a stopped sandbox | 0 | deleted |
| 31 | the same in a non-interactive shell with the project spelled out | 0 | no prompt in either mode |
| 32 | after a destroy, inspect the host | - | host clone, Dockerfile, image, template, workspace and secret are kept |
| 33 | after a destroy, inspect `.sbxm` | - | metadata, lock file and cache are gone |
| 34 | `sbxm open <owner>/<repo>` after a destroy | 1 | the project is no longer managed |
| 35 | `sbxm add <owner>/<repo>` again | 0 | registers as a new project |
| 36 | case 35 with the kept Dockerfile | 0 | the first build uses the Dockerfile that survived the destroy |

### Result

| Item | Value |
|---|---|
| Date | not run yet |
| macOS / arch | |
| `sbx version` | |
| `docker version` | |
| Cases passed | |
| Cases failed | |

### Daemon safety probe

The items of the Phase 2 spec belong here too. Record them the same way.

| # | Question |
|---:|---|
| 1 | A daemon started with `SSH_AUTH_SOCK` forwards the agent into the sandbox |
| 2 | A daemon started with `SSH_AUTH_SOCK` unset does not |
| 3 | A sandbox can be reused or created after the daemon is stopped and started |
