# Modifications

## Positioning

This repository is an **internal modified fork based on the upstream branch**.
It is not a clean mirror of upstream. We keep pulling upstream changes, but this tree contains
internal Xiaohongshu-specific modifications for authentication, packaging, installer behavior,
configuration defaults, and auxiliary scripts.

When syncing with upstream later, treat these changes as **intentional internal divergence**.

## Major Internal Modifications

### 1. Internal SSO + cookie authentication

- Added internal SSO login flow and persisted SSO session handling.
- Current authentication path is based on **SSO + cookie**, not the old OpenAI-auth-only path.
- Internal SSO is **not** gated by `requires_openai_auth`. We decoupled the internal SSO trigger from
  upstream OpenAI login semantics so RedNote / runway-style providers can use SSO + cookie without
  opening the OpenAI login flow.
- Cookie auth is isolated into a dedicated provider implementation to reduce merge conflicts with upstream.
- TUI startup behavior was adjusted so embedded runs can automatically start SSO login when there is no
  valid SSO session and no API-key-based auth in the environment.

Relevant files include:

- `codex-rs/login/src/sso_config.rs`
- `codex-rs/login/src/sso_login.rs`
- `codex-rs/model-provider/src/cookie_auth_provider.rs`
- `codex-rs/model-provider/src/auth.rs`
- `codex-rs/model-provider-info/src/lib.rs`
- `codex-rs/model-provider/src/provider.rs`
- `codex-rs/model-provider/src/models_endpoint.rs`
- `codex-rs/tui/src/lib.rs`
- `codex-rs/core/src/mcp_openai_file.rs`

### 2. Distribution name is `codex-cli`

Upstream still builds the real Rust binary as:

- crate: `codex-cli`
- binary: `codex`

However, for our **internal distribution**, we intentionally ship/install the process name as:

- `codex-cli`

This is currently implemented in the packaging/installer layer, not by renaming the upstream Rust
binary target itself.

#### Current mapping

- Upstream-built binary name: `codex`
- Internal packaged/installed binary name: `codex-cli`

#### If we want to revert later

To revert the distributed process name back to `codex` (or change it to something else), the main
places to inspect are:

- `xhs.sh`
  - `CLI_NAME`
  - package archive contents (`compress_one`)
  - upload artifact naming logic
- `install-codex-cli.sh`
  - `CLI_NAME`
  - extracted binary lookup
  - installed target path
  - help text / user-facing command hints

Important: unless we explicitly choose to do so, **do not assume the upstream Rust `[[bin]] name = "codex"`
should be changed**. Our current strategy is only to rename the distributed artifact/process at the
shell-script layer.

### 3. Configuration directory remains lowercase `.codex`

Our configuration directory should remain:

- `~/.codex`

Do not use `~/.Codex`.

This affects:

- installer output paths
- config examples
- model catalog path examples
- future documentation

### 4. Provider is still meaningful

Even though authentication now uses **SSO + cookie**, the `provider` concept is **not fully obsolete**.

Provider-related configuration still influences runtime behavior such as:

- model routing
- base URL / wire API selection
- model catalog handling
- other provider-scoped configuration

So the right mental model is:

- **OpenAI auth dependency is reduced/replaced for our internal flow**
- **provider is still part of runtime configuration and should not be treated as dead**

## Script Notes

### `xhs.sh`

- Internal build/distribution helper script.
- Assumes internal OSS upload workflow.
- Uses upstream Cargo package `codex-cli` and upstream binary `codex` for compilation.
- Renames the packaged executable to `codex-cli` for internal distribution.

### `install-codex-cli.sh`

- Internal one-click installer for our OSS-hosted artifacts.
- Installs the distributed binary as `codex-cli`.
- Uses lowercase `~/.codex` as configuration directory.
- Downloads `model_catalog.json` into the same lowercase config directory.

## Maintenance Guidance

Before future upstream syncs or installer/distribution changes, re-check at least:

- `codex-rs/cli/Cargo.toml`
- `xhs.sh`
- `install-codex-cli.sh`
- `config.toml.example`
- auth flow changes under `codex-rs/login/`, `codex-rs/model-provider/`, and `codex-rs/tui/`

If later maintainers want to undo any internal branding/packaging change, this file should be used as
the first rollback map.
