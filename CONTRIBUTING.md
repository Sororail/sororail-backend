# Contributing to sororail-backend

Thanks for your interest in contributing!

## Getting started

1. Install Rust 1.89 or newer and PostgreSQL.
2. Fork and clone the repo, then:
   ```sh
   cp .env.example .env
   # set DATABASE_URL, and JWT_SECRET (openssl rand -hex 32)
   cargo run
   ```
3. Migrations in `migrations/` run automatically on startup.

## Picking up work

- Browse open issues. Comment on one to ask to be assigned before starting, so
  two people don't work on the same thing.
- For anything not covered by an existing issue, open an issue first to discuss
  the approach.

## Pull requests

- Keep each PR focused on a single issue, and reference it (`Closes #123`).
- Before pushing, make sure these pass locally (CI runs the same checks):
  ```sh
  cargo fmt --all --check
  cargo clippy --all-targets --locked -- -D warnings
  cargo test --locked
  ```
- Add or update tests for behaviour you change.
- Add a new numbered file under `migrations/` for schema changes; never edit a
  migration that has already been merged.
- Describe what changed and how you tested it in the PR description.

## Code style

- Handlers return `Result<_, AppError>` (see `src/error.rs`) so errors are
  rendered consistently as `{"error": "..."}`.
- Don't log secrets, passwords, or tokens.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
