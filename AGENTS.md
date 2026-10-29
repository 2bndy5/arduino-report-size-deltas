# AGENTS.md

This document provides operational context, development commands, architectural guidance, and repository rules for AI agents and automated contributors working on `arduino-report-size-deltas`.

---

## 1. Project Overview

`arduino-report-size-deltas` compares and reports memory footprint deltas (flash and RAM usage) of Arduino sketches compiled across git commits or branches.

It exists in two primary distribution formats:
1. **Rust Crate / CLI Binary**:
   - Library: `arduino-report-size-deltas` (core parsing and markdown generation logic).
   - CLI application: `report-size-deltas` (feature-gated under the `bin` feature), which parses JSON compile-size reports and posts or updates comments on GitHub PR threads via GitHub REST API.
2. **GitHub Composite Action (`action.yml`)**:
   - Installs pre-compiled `report-size-deltas` release binaries via `cargo-binstall` and invokes the CLI in GitHub Actions workflows.

---

## 2. Repository Layout & Architecture

```
.
├── .config/
│   ├── cliff.toml            # Configuration for git-cliff (changelog generation)
│   ├── committed.toml        # Configuration for committed (conventional commit linting)
│   └── nextest.toml          # Test runner profiles for cargo-nextest
├── .github/
│   ├── workflows/
│   │   ├── bump-n-release.nu # Nushell script for release automation
│   │   ├── bump-n-release.yml# Release workflow (workflow_dispatch) & changelog preview
│   │   ├── pre-commit.yml    # CI job running pre-commit, rust lints, PR title, and workflow linter
│   │   ├── rust.yml          # CI testing, coverage, multi-target cross-compilation, crates.io release
│   │   └── self-test.yml     # Self-test testing action.yml against local sample artifacts
│   ├── requirements.txt      # Python dependencies for CI tooling (git-cliff)
│   └── zizmor.yml            # Configuration/allowlist for zizmor action linter
├── action.yml                # GitHub composite action definition
├── Cargo.toml                # Cargo manifest (Rust 2024 edition)
├── CONTRIBUTING.md           # Human contributor guide
├── cspell.config.yml         # Project spell-checker configuration
├── nurfile                   # Task runner definitions in Nushell for `nur`
├── src/
│   ├── lib.rs                # Public library exports; enforces strict no-panic/no-unwrap clippy lints
│   ├── main.rs               # CLI application entrypoint (requires `bin` feature)
│   ├── error.rs              # Custom error types (using `thiserror`)
│   ├── reports/              # Parsing sketch compilation JSON artifact structures
│   │   ├── mod.rs
│   │   └── structs.rs
│   └── summarize/            # Delta calculation and markdown report formatting
│       ├── helpers.rs
│       ├── mod.rs
│       └── structs.rs
└── tests/
    ├── size-deltas-reports-new/  # Test fixture reports
    ├── size-deltas-reports-old/  # Test fixture reports
    └── test_assets/              # Test fixture JSON files
```

---

## 3. Tooling & Development Environment

The project leverages the following tools:
- **Rust Toolchain**: 2024 edition, stable.
- **`nur`**: Nushell-based task runner for standardized development commands (configured in `nurfile`).
- **`cargo-nextest`**: Test execution engine.
- **`cargo-llvm-cov`**: LLVM-based code coverage analysis.
- **`pre-commit`**: File sanitation and linting (executed via `uvx` or `pipx`).
- **`committed`**: Conventional commit validator.
- **`cspell`**: Spellchecker for markdown, code, and comments.
- **`git-cliff`**: Changelog generator.
- **`zizmor`**: Security and static analysis for GitHub Actions workflows.

---

## 4. Development Commands & Workflows

Agents can use `nur` tasks directly if `nur` is available, or invoke the underlying cargo/uvx tools:

### Building & Checking
- **Check code with binary feature**:
  ```shell
  cargo check --features bin --all-targets
  ```
- **Build CLI binary**:
  ```shell
  cargo build --bin report-size-deltas --features bin
  ```

### Testing

> [!CAUTION]
> **Do NOT run `cargo test` for this project.**
> Some tests mutate process environment variables (`GITHUB_TOKEN`, `GITHUB_ACTIONS`, `GITHUB_EVENT_NAME`, `GITHUB_API_URL`, etc.). Standard `cargo test` executes test threads concurrently within the same process, causing environment variable collisions and random test failures.
>
> **Always use `cargo-nextest`** (which executes each test in an isolated process).

- **Run unit tests with nextest (default profile)**:
  ```shell
  nur test
  # Or directly with cargo-llvm-cov & nextest:
  cargo llvm-cov --no-report nextest --features bin --lib --tests --color always --profile default
  # Or with cargo-nextest directly:
  cargo nextest run --features bin --lib --tests --profile default
  ```
- **Run tests with CI profile** (includes tests running > 10 seconds):
  ```shell
  nur test -p ci
  # Or directly with nextest:
  cargo llvm-cov --no-report nextest --features bin --lib --tests --color always --profile ci
  # Or with cargo-nextest directly:
  cargo nextest run --features bin --lib --tests --profile ci
  ```
- **Generate coverage**:
  - LCOV format (for Codecov / coverage tooling):
    ```shell
    nur test lcov
    # Or directly:
    cargo llvm-cov report --lcov --output-path lcov.info
    ```
  - Local HTML report:
    ```shell
    nur test llvm-cov
    # Or directly:
    cargo llvm-cov report --html
    ```

### Linting & Formatting
- **Clippy and Rustfmt**:
  ```shell
  # Auto-fix mode:
  nur lint
  # Check mode (fails on any warnings or formatting differences):
  nur lint -c
  # Or directly:
  cargo clippy --all-targets --features bin -- -D warnings
  cargo fmt --check
  ```

### Pre-commit & Sanitization
Pre-commit checks line endings, trailing whitespace, EOF newlines, YAML/TOML syntax, file size limits (< 500 kB), and spelling.
- **Run pre-commit**:
  ```shell
  nur pre-commit
  # Or using uvx:
  uvx pre-commit run --all-files
  # Or using pipx:
  pipx run pre-commit run --all-files
  ```
- **Lint GitHub Actions Workflows**:
  ```shell
  pipx run zizmor --format github ./
  ```

### Documentation
- **Build API documentation**:
  ```shell
  nur docs
  # Or directly:
  cargo doc --no-deps --lib
  ```

---

## 5. Coding Standards & Agent Constraints

When modifying code or creating commits, AI agents MUST observe the following rules:

### Strict Error Handling in Library Code
- `src/lib.rs` explicitly enforces:
  ```rust
  #![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
  ```
- **Never** use `.unwrap()`, `.expect()`, or `panic!` within `src/lib.rs` or its library submodules (`reports`, `summarize`, `error`).
- Use `Result` and propagate errors using the crate's `JsonError` or `CommentAssemblyError` (built with `thiserror`).

### Process Isolation for Tests (Environment Variable Collisions)
- CLI tests in `src/main.rs` mock GitHub CI conditions by setting process environment variables (`env::set_var("GITHUB_TOKEN", ...)`).
- **Never execute standard parallel `cargo test`**. Concurrent threads sharing the same process address space overwrite each other's environment variables.
- Use `cargo-nextest` (`nur test` or `cargo nextest run`) as it spawns each test in an independent process.
- If `cargo test` must be invoked directly, always append `-- --test-threads=1`.

### Feature Flags & Modular Dependencies
- CLI-specific dependencies (`clap`, `tokio`, `reqwest`, `git-bot-feedback`, `colored`, `anyhow`) are optional and gated under the `bin` feature.
- Library code must only rely on core unconditional dependencies (`serde`, `serde_json`, `thiserror`, `log`).
- When running check/build/clippy commands, always specify `--features bin` so that `src/main.rs` is included in the validation.

### Line Endings & Formatting
- All text and code files MUST use **LF** (`\n`) line endings. Do NOT introduce CRLF line endings.
- No trailing whitespace and all files must end with a newline character.
- Code formatting must satisfy `cargo fmt --check`.

### Spelling & Terminology (`cspell`)
- CI validates spelling with `cspell-cli`.
- If new microcontroller architectures, board acronyms (e.g., `samd`, `FQBN`), or domain-specific names are introduced, add them to `cspell.config.yml` under `words` (keep list sorted) if flagged.

### Commit Messages & PR Titles
- All pull request titles must comply with [Conventional Commits](https://www.conventionalcommits.org):
  - Allowed types: `fix`, `feat`, `build`, `chore`, `docs`, `style`, `refactor`, `remove`, `deprecate`, `security`, `add`, `perf`, `test`.
  - Subject line must NOT be capitalized (e.g., `feat: support new report format`, not `feat: Support new report format`).
  - PR titles are verified by `committed --config .config/committed.toml` and spellchecked by `cspell`.
- PR branches are squashed into a single commit upon merge to `main`.

### Release Orchestration
- Releases are automated through GitHub Actions (`.github/workflows/bump-n-release.yml`) using the Nushell script `.github/workflows/bump-n-release.nu`.
- The script automatically synchronizes versions across `Cargo.toml`, `action.yml`, and `README.md`, generates release notes with `git-cliff`, and tags commits.
- Do not manually increment version numbers across files unless specifically requested.
