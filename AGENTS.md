# Repository Guidelines

## Project Structure & Module Organization

- `src/` contains the Rust CLI implementation.
- `src/cmd/` holds subcommands and command dispatch (`mod.rs`, `ps.rs`, `exec.rs`).
- `src/who/` provides platform-specific process/user discovery (`linux.rs`, `mac.rs`, `windows.rs`).
- Top-level files include `Cargo.toml`, `Cargo.lock`, `README.md`, and `SPEC.md` (command spec).
- No dedicated `tests/` or `examples/` directories are present.

## Build, Test, and Development Commands

- `cargo build` builds the CLI binary.
- `cargo check` runs a fast type-check without producing artifacts.
- `cargo run -- <args>` runs the CLI locally (example: `cargo run -- ps`).
- There are no repository-provided test commands; add `cargo test` only if tests are introduced.

## Coding Style & Naming Conventions

- Use standard Rust formatting: 4-space indentation, snake_case for functions/modules, PascalCase for types.
- Keep modules small and focused; place CLI parsing in `src/cli.rs` and command logic in `src/cmd/`.
- Prefer clear, descriptive names that mirror CLI verbs (e.g., `ps`, `exec`).
- Run `cargo fmt` when touching Rust files if rustfmt is available.

## Testing Guidelines

- No test framework is currently set up in this repository.
- If you add tests, use Rust’s built-in test harness (`#[test]`) and keep naming consistent with the module under test.
- Place unit tests near the implementation or create a `tests/` directory for integration tests.

## Commit & Pull Request Guidelines

- Git history is empty; no commit message convention is established yet.
- Use clear, imperative commit messages (example: `Add ps watch output formatting`).
- PRs should include a short summary, relevant `SPEC.md` references, and manual test notes (e.g., `cargo run -- ps`).

## Security & Configuration Tips

- This tool wraps `git worktree`; avoid running commands on untrusted repositories.
- Keep platform-specific code isolated under `src/who/` to prevent cross-OS regressions.
