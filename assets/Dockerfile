# Template for a Docker Sandbox managed by sbxm.
#
# sbxm writes this file once, when a project is registered, and never edits it
# again. Change it here and run `sbxm rebuild <owner>/<repository>` to apply the
# change to the sandbox.
#
# Rules this file has to keep:
#   - the base image stays pinned by digest
#   - every tool it installs names its version in an ARG and fetches that
#   - no secret value is written into the image
#   - no COPY or ADD: sbxm builds with an empty build context

FROM docker.io/docker/sandbox-templates:shell-docker@sha256:39cf20eca861ec92747487af6197f6d916f774bdb98245d267dbd8dfd3debb05

USER root

# The fixed tool set. Being part of the base image is not treated as proof that
# a tool is installed, so every one of them is installed here.
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates \
      coreutils \
      curl \
      git \
      jq \
      openssh-client \
      wget \
 && rm -rf /var/lib/apt/lists/*

ARG GH_VERSION=2.96.0
RUN set -eux; \
    architecture="$(dpkg --print-architecture)"; \
    curl -fsSL -o /tmp/gh.tar.gz \
      "https://github.com/cli/cli/releases/download/v${GH_VERSION}/gh_${GH_VERSION}_linux_${architecture}.tar.gz"; \
    tar -xzf /tmp/gh.tar.gz -C /tmp; \
    install -m 0755 "/tmp/gh_${GH_VERSION}_linux_${architecture}/bin/gh" /usr/local/bin/gh; \
    rm -rf /tmp/gh.tar.gz "/tmp/gh_${GH_VERSION}_linux_${architecture}"; \
    gh --version

ARG CLAUDE_CODE_VERSION=2.1.220
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) slice="linux-x64" ;; \
      arm64) slice="linux-arm64" ;; \
      *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/claude.tar.gz \
      "https://github.com/anthropics/claude-code/releases/download/v${CLAUDE_CODE_VERSION}/claude-${slice}.tar.gz"; \
    tar -xzf /tmp/claude.tar.gz -C /tmp; \
    install -m 0755 /tmp/claude /usr/local/bin/claude; \
    rm -f /tmp/claude.tar.gz /tmp/claude; \
    claude --version

# The release tag carries a `rust-` prefix that the version itself does not, and
# the tarball holds the binary under its target triple rather than under `codex`.
ARG CODEX_VERSION=0.145.0
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) target="x86_64-unknown-linux-musl" ;; \
      arm64) target="aarch64-unknown-linux-musl" ;; \
      *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/codex.tar.gz \
      "https://github.com/openai/codex/releases/download/rust-v${CODEX_VERSION}/codex-${target}.tar.gz"; \
    tar -xzf /tmp/codex.tar.gz -C /tmp; \
    install -m 0755 "/tmp/codex-${target}" /usr/local/bin/codex; \
    rm -f /tmp/codex.tar.gz "/tmp/codex-${target}"; \
    codex --version

# The workspace root every managed worktree lives under.
RUN install -d -o agent -g agent -m 0755 /home/agent/work

USER agent

ARG MISE_VERSION=v2026.7.15
RUN curl -fsSL https://mise.run | MISE_VERSION="${MISE_VERSION}" sh
ENV PATH=/home/agent/.local/bin:/home/agent/.local/share/mise/shims:${PATH}

WORKDIR /home/agent/work
