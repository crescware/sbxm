---
title: Create your first sandbox
description: Register a GitHub project, protect its credential, and open its Docker Sandbox.
---

This walkthrough creates the host-side project artifacts first and builds the sandbox only after its GitHub credential is registered.

## 1. Check the host

```sh
sbxm status --global
```

Fix any reported requirement before continuing. sbxm refuses to continue when it cannot observe a safe host state.

## 2. Declare your Git identity

If you have not already configured a Git identity, set the values you want to use as the starting text for the first interactive registration:

```sh
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

sbxm asks which name and email the project commits use. Press Enter twice to accept the displayed values, or type different values. The choice is saved as sbxm’s default; each registered project keeps the identity it was registered with.

## 3. Register the project

Change to the parent directory where the project should live, then pass the GitHub clone URL unchanged:

```sh
cd ~/Projects
sbxm add git@github.com:<owner>/<repository>.git
```

HTTPS is also accepted:

```sh
sbxm add https://github.com/<owner>/<repository>.git
```

`sbxm add` accepts only these SSH and HTTPS GitHub clone URL forms. It creates `<repository>.project/` in the directory where you run it, creates a host clone and Dockerfile, and prints the project ID, sandbox name, and next commands. It does not build the sandbox yet.

The first interactive run also asks for the display language and project Git identity. In a non-interactive environment, declare both identity values explicitly:

```sh
sbxm add git@github.com:<owner>/<repository>.git \
  --git-user-name '<name>' --git-user-email '<email>'
```

## 4. Register the GitHub credential

`sbxm add` prints a project-specific command like this:

```sh
sbx secret set-custom <sandbox> \
  --host github.com \
  --host '**.github.com' \
  --host '**.githubusercontent.com' \
  --host ghcr.io \
  --env GH_TOKEN \
  --value <token>
```

Replace `<sandbox>` and `<token>` with the values from your project setup. The real token remains with the Docker Sandboxes secret proxy. Do not commit it, put it in `config.yaml`, or paste it into a public issue.

## 5. Prepare and open

```sh
sbxm prepare <project-id>
sbxm open <project-id>
```

`prepare` builds the project image, creates the sandbox, clones the repository inside it, and creates the managed worktrees. `open` starts a stopped sandbox when necessary and connects over SSH.

The session starts in `/home/agent/work/<repository>`. To start in a managed worktree, use its zero-based index, for example `sbxm open <project-id> -i 0`.

Managed worktrees are located at paths like:

```text
/home/agent/work/<repository>/<repository>.tree-1
/home/agent/work/<repository>/<repository>.tree-2
```

## More control

Use [managed worktrees](../../guides/worktrees/) for independent tasks, [customize the sandbox image](../../guides/custom-image/) when the generated Dockerfile needs tools, and [tear down safely](../../guides/teardown/) when a project is no longer managed.
