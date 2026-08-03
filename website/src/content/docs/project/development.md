---
title: Development
description: Build and check sbxm from source using the repository development toolchain.
---

The Rust CLI development environment is managed by the repository-root `mise.toml`. From the repository root:

```sh
mise install
mise run check
```

The check task formats, lints, verifies the macOS target, runs tests, and checks coverage. See [docs/development.md](https://github.com/crescware/sbxm/blob/main/docs/development.md) for the project conventions.

The website has its own `website/mise.toml` for Node and pnpm. Run website commands from `website/` and keep the two toolchains separate.
