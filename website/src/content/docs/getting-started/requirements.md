---
title: Requirements
description: The host, Docker, Git, SSH, and GitHub requirements for sbxm.
---

sbxm currently supports **macOS 14 or later on Apple silicon**.

## Required tools

- macOS 14 or later on an arm64 Mac
- Docker Desktop with a running Docker Engine
- Docker Sandboxes CLI 0.37.0 or later
- Git and SSH
- A GitHub personal access token with read and write access to each repository you manage

Run the following command after installation to check the host and Docker Sandboxes environment:

```sh
sbxm status --global
```

The global status command is read-only. It reports missing commands, an unavailable Docker daemon, an unsupported platform, Docker Sandboxes login state, and the SSH setup needed to connect to a sandbox.

## GitHub token permissions

For a fine-grained token, grant **Contents: read and write** and **Metadata: read** for the repository. A classic token needs the `repo` scope.

The token is registered with Docker Sandboxes’ secret proxy. It is not copied into the sandbox or written into a project configuration file. See [Create your first sandbox](../quickstart/) for the registration command.

## Platform scope

sbxm validates the platform it supports instead of guessing that another operating system or architecture will work. If the host is outside this scope, the CLI stops before changing project state.
