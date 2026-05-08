# Internal Fork Context (Xiaohongshu) / 内部分支背景（先读）

> This repository is an **internal modified fork** based on upstream `openai/codex` and is **not** a clean mirror.
> 本仓库是基于 upstream 的内部改造分支，不是纯同步镜像。

## 0. Scope and intent / 作用范围

- Keep pulling upstream changes, but preserve intentional internal divergence in auth, packaging, installer defaults, and scripts.
- Future merges/rebases must treat this divergence as deliberate, not accidental drift.

## 1. Desktop GUI reality (closed source) / 桌面 GUI 现实（闭源）

- `codex app`/`codex-cli ... app` only acts as launcher/installer.
- The GUI itself is a closed-source desktop app bundle (`/Applications/Codex.app`).
- After GUI starts, it launches its own bundled runtime binary, typically:
  - `/Applications/Codex.app/Contents/Resources/codex app-server --analytics-default-enabled`
- Therefore, GUI runtime execution does **not** directly run the local custom `codex-cli` binary.

### 1.1 Practical startup flow / 实际启动链路

1. User runs launcher command: `codex app` or `codex-cli --env <env> app`.
2. Launcher opens/installs Codex Desktop app.
3. GUI process starts (`/Applications/Codex.app/Contents/MacOS/Codex`).
4. GUI spawns bundled app-server binary under app bundle resources.

### 1.2 Runtime verification guidance / 运行时核验指引

- Verify process path with `ps/pgrep`:
  - look for `.../Contents/Resources/codex app-server ...`
- Verify which config/state files are opened with `lsof -p <pid>`:
  - look for `~/.codex/config.toml`, sqlite, sessions, etc.
- Verify outbound connection target with `lsof -nP -a -p <pid> -iTCP -sTCP:ESTABLISHED` and desktop logs.

### 1.3 Observed behavior in this repo context / 本仓库场景下的已观测事实

- Bundled GUI app-server process path observed:
  - `/Applications/Codex.app/Contents/Resources/codex app-server --analytics-default-enabled`
- It opened config/state under `~/.codex`, including:
  - `~/.codex/config.toml`
  - `~/.codex/state_*.sqlite*`
  - `~/.codex/logs_*.sqlite*`
  - `~/.codex/sessions/.../rollout-*.jsonl`
- Local `~/.codex/config.toml` values influence runtime behavior (provider/base_url/auth-related settings).
- So the operational model is:
  - **Binary source** = GUI bundled closed-source runtime
  - **Behavior/config source** = local `~/.codex` runtime config/state

## 2. Internal modifications (canonicalized from former Modifications.md) / 内部改造总览

### 2.1 Authentication direction (historical notes must be read together)

There are two historical snapshots in branch docs; keep both for maintenance context:

- Snapshot A (from earlier internal notes): login first, then API key from config file; internal login decoupled from upstream `requires_openai_auth` gating.
- Snapshot B (from code-guide snapshot): internal SSO + cookie provider path emphasized.

When touching auth, validate **current code truth** in these modules before changing behavior:

- `codex-rs/login/src/sso_config.rs`
- `codex-rs/login/src/sso_login.rs`
- `codex-rs/login/src/auth/manager.rs`
- `codex-rs/model-provider/src/auth.rs`
- `codex-rs/model-provider/src/provider.rs`
- `codex-rs/model-provider/src/models_endpoint.rs`
- If present in current branch: `codex-rs/model-provider/src/cookie_auth_provider.rs`

### 2.2 Internal login conflict isolation strategy

To reduce repeated upstream merge conflicts in high-churn login files:

- Keep `codex-rs/cli/src/login.rs` as close to upstream as practical.
- Put internal SSO entry behavior in `codex-rs/cli/src/login_sso_internal.rs`.
- Export internal login/status/logout through `codex-rs/cli/src/lib.rs`.

Current expected exported entry names include:

- `run_sso_login`
- `run_login_status`
- `run_logout`

### 2.3 CLI behavior and env

- CLI login default path should use internal SSO wiring where this branch enables it.
- `--env` (e.g. `prod/sit`) is part of runtime SSO environment selection in this fork.

### 2.4 Distribution naming policy

- Upstream Cargo binary remains `codex` (crate `codex-cli`).
- Internal distribution intentionally ships/installs as `codex-cli`.
- This rename is done in packaging/installer layer (scripts), not by changing upstream Rust bin target naming.

Primary files:

- `xhs.sh`
- `install-codex-cli.sh`

### 2.5 Config directory policy

- Config directory remains lowercase `~/.codex`.
- Do not change to `~/.Codex` unless explicitly planned and fully migrated.

### 2.6 Provider semantics

- Provider is still meaningful (routing/base_url/catalog/runtime behavior), even when auth source strategy is customized.
- Do not treat provider config as dead code.

## 3. Module-level divergence snapshot / 模块差异快照

> This is a maintenance map; always confirm with `git diff origin/main...HEAD` before risky refactors.

### 3.1 CLI

- `M codex-rs/cli/src/lib.rs`
- `A codex-rs/cli/src/login_sso_internal.rs`
- `M codex-rs/cli/src/main.rs`

### 3.2 Login

- `M codex-rs/login/src/auth/manager.rs`
- `M codex-rs/login/src/lib.rs`
- `A codex-rs/login/src/sso_config.rs`
- `A codex-rs/login/src/sso_login.rs`

### 3.3 Model Provider

- `M codex-rs/model-provider-info/src/lib.rs`
- `M codex-rs/model-provider/src/auth.rs`
- `M codex-rs/model-provider/src/lib.rs`
- `M codex-rs/model-provider/src/models_endpoint.rs`
- `M codex-rs/model-provider/src/provider.rs`
- Historical note: some snapshots include `A codex-rs/model-provider/src/cookie_auth_provider.rs`

### 3.4 TUI

- Internal auth/status integration touched `tui/src/lib.rs`, `tui/src/app.rs`, `tui/src/chatwidget.rs` and related tests/snapshots.

### 3.5 Scripts / Distribution / Other

- `A xhs.sh`
- `A install-codex-cli.sh`
- `A model_catalog.json`
- `M codex-rs/codex-api/src/auth.rs`
- `M codex-rs/core/src/mcp_openai_file.rs`

## 4. Recommended reading path (from 代码导读 + Modifications) / 推荐阅读顺序

1. Entry and routing:
   - `codex-rs/cli/src/main.rs`
   - `codex-rs/cli/src/lib.rs`
   - `codex-rs/cli/src/login_sso_internal.rs`
2. Auth implementation:
   - `codex-rs/login/src/sso_config.rs`
   - `codex-rs/login/src/sso_login.rs`
   - `codex-rs/login/src/auth/manager.rs`
3. Provider/auth propagation:
   - `codex-rs/model-provider/src/auth.rs`
   - `codex-rs/model-provider/src/provider.rs`
   - `codex-rs/model-provider/src/models_endpoint.rs`
   - `codex-rs/model-provider/src/cookie_auth_provider.rs` (if present)
4. UI behavior:
   - `codex-rs/tui/src/lib.rs`
   - `codex-rs/tui/src/app.rs`
   - `codex-rs/tui/src/chatwidget.rs`
5. Packaging/install:
   - `xhs.sh`
   - `install-codex-cli.sh`
   - `model_catalog.json`

## 5. Merge/maintenance guardrails / 维护与合并指引

1. Prefer extending internal logic in `login_sso_internal.rs` over editing upstream high-churn `login.rs`.
2. During upstream sync, verify `cli/src/lib.rs` exports still point to intended internal handlers.
3. After auth-chain changes, at minimum run:
   - `cd codex-rs && just fmt`
   - `cd codex-rs && cargo test -p codex-cli`
4. Keep rollback steps file-scoped; avoid bundling unrelated behavior changes in one rollback commit.

## 6. Rollback map / 回滚地图

1. Auth rollback:
   - Revert internal login/config callsites and restore upstream wiring for CLI/TUI/model-provider entrypoints.
   - Primary files:
     - `codex-rs/cli/src/login_sso_internal.rs`
     - `codex-rs/login/src/sso_config.rs`
     - `codex-rs/login/src/sso_login.rs`
2. Distribution naming rollback:
   - Revert `codex-cli` packaging/install naming in `xhs.sh` and `install-codex-cli.sh`.
3. Config-path rollback (careful):
   - Audit all `~/.codex` references before changing directory policy.
4. Safe rollback procedure:
   - Use file-scoped revert commits.
   - Re-run formatting/tests for touched crates.
   - Validate installer behavior separately.

## 7. 30-minute onboarding (from 代码导读) / 新同学 30 分钟上手

- 0-5 min:
  - `README.md`
  - This `AGENTS.md` section (internal fork context)
- 5-12 min:
  - CLI login routing (`main.rs`, `lib.rs`, `login_sso_internal.rs`)
- 12-20 min:
  - Auth/provider chain (`sso_config.rs`, `sso_login.rs`, provider auth files)
- 20-26 min:
  - TUI consumption of auth state (`tui/src/lib.rs`, `app.rs`, `chatwidget.rs`)
- 26-30 min:
  - Packaging/install chain (`xhs.sh`, `install-codex-cli.sh`, `model_catalog.json`)

---

# Rust/codex-rs

In the codex-rs folder where the rust code lives:

- Crate names are prefixed with `codex-`. For example, the `core` folder's crate is named `codex-core`
- When using format! and you can inline variables into {}, always do that.
- Install any commands the repo relies on (for example `just`, `rg`, or `cargo-insta`) if they aren't already available before running instructions here.
- Never add or modify any code related to `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` or `CODEX_SANDBOX_ENV_VAR`.
  - You operate in a sandbox where `CODEX_SANDBOX_NETWORK_DISABLED=1` will be set whenever you use the `shell` tool. Any existing code that uses `CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR` was authored with this fact in mind. It is often used to early exit out of tests that the author knew you would not be able to run given your sandbox limitations.
  - Similarly, when you spawn a process using Seatbelt (`/usr/bin/sandbox-exec`), `CODEX_SANDBOX=seatbelt` will be set on the child process. Integration tests that want to run Seatbelt themselves cannot be run under Seatbelt, so checks for `CODEX_SANDBOX=seatbelt` are also often used to early exit out of tests, as appropriate.
- Always collapse if statements per https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if
- Always inline format! args when possible per https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args
- Use method references over closures when possible per https://rust-lang.github.io/rust-clippy/master/index.html#redundant_closure_for_method_calls
- Avoid bool or ambiguous `Option` parameters that force callers to write hard-to-read code such as `foo(false)` or `bar(None)`. Prefer enums, named methods, newtypes, or other idiomatic Rust API shapes when they keep the callsite self-documenting.
- When you cannot make that API change and still need a small positional-literal callsite in Rust, follow the `argument_comment_lint` convention:
  - Use an exact `/*param_name*/` comment before opaque literal arguments such as `None`, booleans, and numeric literals when passing them by position.
  - Do not add these comments for string or char literals unless the comment adds real clarity; those literals are intentionally exempt from the lint.
  - The parameter name in the comment must exactly match the callee signature.
  - You can run `just argument-comment-lint` to run the lint check locally. This is powered by Bazel, so running it the first time can be slow if Bazel is not warmed up, though incremental invocations should take <15s. Most of the time, it is best to update the PR and let CI take responsibility for checking this (or run it asynchronously in the background after submitting the PR). Note CI checks all three platforms, which the local run does not.
- When possible, make `match` statements exhaustive and avoid wildcard arms.
- Newly added traits should include doc comments that explain their role and how implementations are expected to use them.
- When writing tests, prefer comparing the equality of entire objects over fields one by one.
- When making a change that adds or changes an API, ensure that the documentation in the `docs/` folder is up to date if applicable.
- Prefer private modules and explicitly exported public crate API.
- If you change `ConfigToml` or nested config types, run `just write-config-schema` to update `codex-rs/core/config.schema.json`.
- When working with MCP tool calls, prefer using `codex-rs/codex-mcp/src/mcp_connection_manager.rs` to handle mutation of tools and tool calls. Aim to minimize the footprint of changes and leverage existing abstractions rather than plumbing code through multiple levels of function calls.
- If you change Rust dependencies (`Cargo.toml` or `Cargo.lock`), run `just bazel-lock-update` from the
  repo root to refresh `MODULE.bazel.lock`, and include that lockfile update in the same change.
- After dependency changes, run `just bazel-lock-check` from the repo root so lockfile drift is caught
  locally before CI.
- Bazel does not automatically make source-tree files available to compile-time Rust file access. If
  you add `include_str!`, `include_bytes!`, `sqlx::migrate!`, or similar build-time file or
  directory reads, update the crate's `BUILD.bazel` (`compile_data`, `build_script_data`, or test
  data) or Bazel may fail even when Cargo passes.
- Do not create small helper methods that are referenced only once.
- Avoid large modules:
  - Prefer adding new modules instead of growing existing ones.
  - Target Rust modules under 500 LoC, excluding tests.
  - If a file exceeds roughly 800 LoC, add new functionality in a new module instead of extending
    the existing file unless there is a strong documented reason not to.
  - This rule applies especially to high-touch files that already attract unrelated changes, such
    as `codex-rs/tui/src/app.rs`, `codex-rs/tui/src/bottom_pane/chat_composer.rs`,
    `codex-rs/tui/src/bottom_pane/footer.rs`, `codex-rs/tui/src/chatwidget.rs`,
    `codex-rs/tui/src/bottom_pane/mod.rs`, and similarly central orchestration modules.
  - When extracting code from a large module, move the related tests and module/type docs toward
    the new implementation so the invariants stay close to the code that owns them.
  - Avoid adding new standalone methods to `codex-rs/tui/src/chatwidget.rs` unless the change is
    trivial; prefer new modules/files and keep `chatwidget.rs` focused on orchestration.
- When running Rust commands (e.g. `just fix` or `cargo test`) be patient with the command and never try to kill them using the PID. Rust lock can make the execution slow, this is expected.

Run `just fmt` (in `codex-rs` directory) automatically after you have finished making Rust code changes; do not ask for approval to run it. Additionally, run the tests:

1. Run the test for the specific project that was changed. For example, if changes were made in `codex-rs/tui`, run `cargo test -p codex-tui`.
2. Once those pass, if any changes were made in common, core, or protocol, run the complete test suite with `cargo test` (or `just test` if `cargo-nextest` is installed). Avoid `--all-features` for routine local runs because it expands the build matrix and can significantly increase `target/` disk usage; use it only when you specifically need full feature coverage. project-specific or individual tests can be run without asking the user, but do ask the user before running the complete test suite.

Before finalizing a large change to `codex-rs`, run `just fix -p <project>` (in `codex-rs` directory) to fix any linter issues in the code. Prefer scoping with `-p` to avoid slow workspace‑wide Clippy builds; only run `just fix` without `-p` if you changed shared crates. Do not re-run tests after running `fix` or `fmt`.

## The `codex-core` crate

Over time, the `codex-core` crate (defined in `codex-rs/core/`) has become bloated because it is the largest crate, so it is often easier to add something new to `codex-core` rather than refactor out the library code you need so your new code neither takes a dependency on, nor contributes to the size of, `codex-core`.

To that end: **resist adding code to codex-core**!

Particularly when introducing a new concept/feature/API, before adding to `codex-core`, consider whether:

- There is an existing crate other than `codex-core` that is an appropriate place for your new code to live.
- It is time to introduce a new crate to the Cargo workspace for your new functionality. Refactor existing code as necessary to make this happen.

Likewise, when reviewing code, do not hesitate to push back on PRs that would unnecessarily add code to `codex-core`.

## TUI style conventions

See `codex-rs/tui/styles.md`.

## TUI code conventions

- Use concise styling helpers from ratatui’s Stylize trait.
  - Basic spans: use "text".into()
  - Styled spans: use "text".red(), "text".green(), "text".magenta(), "text".dim(), etc.
  - Prefer these over constructing styles with `Span::styled` and `Style` directly.
  - Example: patch summary file lines
    - Desired: vec!["  └ ".into(), "M".red(), " ".dim(), "tui/src/app.rs".dim()]

### TUI Styling (ratatui)

- Prefer Stylize helpers: use "text".dim(), .bold(), .cyan(), .italic(), .underlined() instead of manual Style where possible.
- Prefer simple conversions: use "text".into() for spans and vec![…].into() for lines; when inference is ambiguous (e.g., Paragraph::new/Cell::from), use Line::from(spans) or Span::from(text).
- Computed styles: if the Style is computed at runtime, using `Span::styled` is OK (`Span::from(text).set_style(style)` is also acceptable).
- Avoid hardcoded white: do not use `.white()`; prefer the default foreground (no color).
- Chaining: combine helpers by chaining for readability (e.g., url.cyan().underlined()).
- Single items: prefer "text".into(); use Line::from(text) or Span::from(text) only when the target type isn’t obvious from context, or when using .into() would require extra type annotations.
- Building lines: use vec![…].into() to construct a Line when the target type is obvious and no extra type annotations are needed; otherwise use Line::from(vec![…]).
- Avoid churn: don’t refactor between equivalent forms (Span::styled ↔ set_style, Line::from ↔ .into()) without a clear readability or functional gain; follow file‑local conventions and do not introduce type annotations solely to satisfy .into().
- Compactness: prefer the form that stays on one line after rustfmt; if only one of Line::from(vec![…]) or vec![…].into() avoids wrapping, choose that. If both wrap, pick the one with fewer wrapped lines.

### Text wrapping

- Always use textwrap::wrap to wrap plain strings.
- If you have a ratatui Line and you want to wrap it, use the helpers in tui/src/wrapping.rs, e.g. word_wrap_lines / word_wrap_line.
- If you need to indent wrapped lines, use the initial_indent / subsequent_indent options from RtOptions if you can, rather than writing custom logic.
- If you have a list of lines and you need to prefix them all with some prefix (optionally different on the first vs subsequent lines), use the `prefix_lines` helper from line_utils.

## Tests

### Snapshot tests

This repo uses snapshot tests (via `insta`), especially in `codex-rs/tui`, to validate rendered output.

**Requirement:** any change that affects user-visible UI (including adding new UI) must include
corresponding `insta` snapshot coverage (add a new snapshot test if one doesn't exist yet, or
update the existing snapshot). Review and accept snapshot updates as part of the PR so UI impact
is easy to review and future diffs stay visual.

When UI or text output changes intentionally, update the snapshots as follows:

- Run tests to generate any updated snapshots:
  - `cargo test -p codex-tui`
- Check what’s pending:
  - `cargo insta pending-snapshots -p codex-tui`
- Review changes by reading the generated `*.snap.new` files directly in the repo, or preview a specific file:
  - `cargo insta show -p codex-tui path/to/file.snap.new`
- Only if you intend to accept all new snapshots in this crate, run:
  - `cargo insta accept -p codex-tui`

If you don’t have the tool:

- `cargo install cargo-insta`

### Test assertions

- Tests should use pretty_assertions::assert_eq for clearer diffs. Import this at the top of the test module if it isn't already.
- Prefer deep equals comparisons whenever possible. Perform `assert_eq!()` on entire objects, rather than individual fields.
- Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.

### Spawning workspace binaries in tests (Cargo vs Bazel)

- Prefer `codex_utils_cargo_bin::cargo_bin("...")` over `assert_cmd::Command::cargo_bin(...)` or `escargot` when tests need to spawn first-party binaries.
  - Under Bazel, binaries and resources may live under runfiles; use `codex_utils_cargo_bin::cargo_bin` to resolve absolute paths that remain stable after `chdir`.
- When locating fixture files or test resources under Bazel, avoid `env!("CARGO_MANIFEST_DIR")`. Prefer `codex_utils_cargo_bin::find_resource!` so paths resolve correctly under both Cargo and Bazel runfiles.

### Integration tests (core)

- Prefer the utilities in `core_test_support::responses` when writing end-to-end Codex tests.

- All `mount_sse*` helpers return a `ResponseMock`; hold onto it so you can assert against outbound `/responses` POST bodies.
- Use `ResponseMock::single_request()` when a test should only issue one POST, or `ResponseMock::requests()` to inspect every captured `ResponsesRequest`.
- `ResponsesRequest` exposes helpers (`body_json`, `input`, `function_call_output`, `custom_tool_call_output`, `call_output`, `header`, `path`, `query_param`) so assertions can target structured payloads instead of manual JSON digging.
- Build SSE payloads with the provided `ev_*` constructors and the `sse(...)`.
- Prefer `wait_for_event` over `wait_for_event_with_timeout`.
- Prefer `mount_sse_once` over `mount_sse_once_match` or `mount_sse_sequence`

- Typical pattern:

  ```rust
  let mock = responses::mount_sse_once(&server, responses::sse(vec![
      responses::ev_response_created("resp-1"),
      responses::ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
      responses::ev_completed("resp-1"),
  ])).await;

  codex.submit(Op::UserTurn { ... }).await?;

  // Assert request body if needed.
  let request = mock.single_request();
  // assert using request.function_call_output(call_id) or request.json_body() or other helpers.
  ```

## App-server API Development Best Practices

These guidelines apply to app-server protocol work in `codex-rs`, especially:

- `app-server-protocol/src/protocol/common.rs`
- `app-server-protocol/src/protocol/v2.rs`
- `app-server/README.md`

### Core Rules

- All active API development should happen in app-server v2. Do not add new API surface area to v1.
- Follow payload naming consistently:
  `*Params` for request payloads, `*Response` for responses, and `*Notification` for notifications.
- Expose RPC methods as `<resource>/<method>` and keep `<resource>` singular (for example, `thread/read`, `app/list`).
- Always expose fields as camelCase on the wire with `#[serde(rename_all = "camelCase")]` unless a tagged union or explicit compatibility requirement needs a targeted rename.
- Exception: config RPC payloads are expected to use snake_case to mirror config.toml keys (see the config read/write/list APIs in `app-server-protocol/src/protocol/v2.rs`).
- Always set `#[ts(export_to = "v2/")]` on v2 request/response/notification types so generated TypeScript lands in the correct namespace.
- Never use `#[serde(skip_serializing_if = "Option::is_none")]` for v2 API payload fields.
  Exception: client->server requests that intentionally have no params may use:
  `params: #[ts(type = "undefined")] #[serde(skip_serializing_if = "Option::is_none")] Option<()>`.
- Keep Rust and TS wire renames aligned. If a field or variant uses `#[serde(rename = "...")]`, add matching `#[ts(rename = "...")]`.
- For discriminated unions, use explicit tagging in both serializers:
  `#[serde(tag = "type", ...)]` and `#[ts(tag = "type", ...)]`.
- Prefer plain `String` IDs at the API boundary (do UUID parsing/conversion internally if needed).
- Timestamps should be integer Unix seconds (`i64`) and named `*_at` (for example, `created_at`, `updated_at`, `resets_at`).
- For experimental API surface area:
  use `#[experimental("method/or/field")]`, derive `ExperimentalApi` when field-level gating is needed, and use `inspect_params: true` in `common.rs` when only some fields of a method are experimental.

### Client->server request payloads (`*Params`)

- Every optional field must be annotated with `#[ts(optional = nullable)]`. Do not use `#[ts(optional = nullable)]` outside client->server request payloads (`*Params`).
- Optional collection fields (for example `Vec`, `HashMap`) must use `Option<...>` + `#[ts(optional = nullable)]`. Do not use `#[serde(default)]` to model optional collections, and do not use `skip_serializing_if` on v2 payload fields.
- When you want omission to mean `false` for boolean fields, use `#[serde(default, skip_serializing_if = "std::ops::Not::not")] pub field: bool` over `Option<bool>`.
- For new list methods, implement cursor pagination by default:
  request fields `pub cursor: Option<String>` and `pub limit: Option<u32>`,
  response fields `pub data: Vec<...>` and `pub next_cursor: Option<String>`.

### Development Workflow

- Update docs/examples when API behavior changes (at minimum `app-server/README.md`).
- Regenerate schema fixtures when API shapes change:
  `just write-app-server-schema`
  (and `just write-app-server-schema --experimental` when experimental API fixtures are affected).
- Validate with `cargo test -p codex-app-server-protocol`.
- Avoid boilerplate tests that only assert experimental field markers for individual
  request fields in `common.rs`; rely on schema generation/tests and behavioral coverage instead.
