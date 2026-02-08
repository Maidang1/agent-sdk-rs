# Repository Guidelines

## Project Structure & Module Organization
- `src/` contains the SDK and binary entrypoint.
- `src/agent/` implements agent behavior and runtime options.
- `src/tool/` contains tool traits, registry, parser, and execution flow.
- `src/provider/` contains provider interfaces and the OpenRouter implementation.
- `examples/` contains runnable end-to-end usage samples (calculator, events, validation, multi-tool).
- `docs/` contains design and implementation notes; treat these as reference, not source of truth.

## Build, Test, and Development Commands
- `cargo check` performs a fast compile check during iteration.
- `cargo build` compiles library and binary artifacts.
- `cargo test --all-targets` runs unit tests and example test targets.
- `cargo run --example simple_events` runs a representative integration-style sample.
- `cargo fmt --all` formats code; `cargo fmt --all -- --check` validates formatting in CI.

## Coding Style & Naming Conventions
- Follow Rust 2021 idioms and use 4-space indentation.
- Use `snake_case` for functions/modules/files, `PascalCase` for structs/enums/traits, `SCREAMING_SNAKE_CASE` for constants.
- Keep modules focused by domain (`agent`, `tool`, `provider`) and prefer small, composable functions.
- Run `cargo fmt --all` before opening a PR; current repository state may include pre-existing formatting drift.

## Testing Guidelines
- Primary test flow is `cargo test --all-targets` plus targeted example runs.
- Place unit tests inline with modules using `#[cfg(test)]` and async tests using `#[tokio::test]` where needed.
- Name tests by behavior, for example: `returns_error_on_missing_required_parameter`.
- For provider-dependent flows, use `OPEN_ROUTER_API_KEY` and run examples locally to validate end-to-end behavior.

## Commit & Pull Request Guidelines
- Recent history follows Conventional Commit style (`feat:`, `feat(core):`); use this consistently and keep subjects imperative.
- Keep commits focused and logically scoped (one concern per commit).
- PRs should include: summary of behavior changes, affected modules (for example `src/tool/parser.rs`), test/command evidence, and linked issue(s).
- Include command output snippets when changing runtime behavior (for example `cargo test --all-targets`).
