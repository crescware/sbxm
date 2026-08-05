---
title: Development
description: Build and check sbxm from source using the repository development toolchain.
---

The Rust CLI development environment is managed by the repository-root `mise.toml`. From the repository root:

```sh
mise install
```

Build and inspect the release binary with:

```sh
cargo build --release
./target/release/sbxm --help
```

To run the checked-out source directly, pass sbxm arguments after `cargo run --`:

```sh
cargo run -- status --global
```

`status --global`, `prepare`, and `open` exercise the host environment and Docker Sandboxes. Run them on a supported macOS 14 or later Apple silicon host with Docker Desktop and the Docker Sandboxes CLI. Linux is supported for compilation and tests, but not for opening a sandbox.

Before proposing a change, run the complete verification task:

```sh
mise run check
```

The check task formats, lints, verifies the macOS target, runs tests, checks coverage, and tests the release script. See [docs/development.md](https://github.com/crescware/sbxm/blob/main/docs/development.md) for the project conventions.

The website has its own `website/mise.toml` for Node and pnpm. Run website commands from `website/` and keep the two toolchains separate.
