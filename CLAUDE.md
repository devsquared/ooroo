# CLAUDE

Guidelines for working on ooroo.

## Code Style

### Error Handling
- Use `anyhow` for application errors (`anyhow::Result`, `anyhow::bail!`, `anyhow::Context`)
- Create new custom errors with `thiserror` when the error is specific and especially repeated
- Add context to errors: `.context("failed to open database")`
- Reserve `thiserror` for library-style typed errors only if needed later

### Idiomatic Rust
- Prefer iterators over manual loops
- Use `?` for early returns, not `.unwrap()` (except in tests)
- Favor `impl Into<T>` and `AsRef<T>` for flexible APIs
- Use `Default` trait where appropriate
- Destructure structs and enums explicitly
- Keep functions small and focused
- Aim for directory structure that is easy to navigate and maintain. This means organizing files by domain and keeping related files together.

### Comments
- Only comment *why*, not *what*
- Complex logic or non-obvious decisions get comments
- No commented-out code
- No obvious comments like `// create a new goal`

### Logging
- Use `tracing` crate for structured logging
- Add logging at key decision points and state transitions
- Use appropriate levels:
  - `error!` — something failed
  - `warn!` — something unexpected but recoverable
  - `info!` — key operations (goal created, task completed)
  - `debug!` — detailed flow for debugging
  - `trace!` — very verbose, rarely used

Example:
```rust
use tracing::{info, debug, error};

info!(goal_id = %id, "goal created");
debug!(task_id = %id, state = ?new_state, "task state transition");
error!(error = ?e, "failed to write to database");
```

## Documentation
- When writing longer form documentation, use Markdown and be well-formatted.
- Do not use emojis.
- Aim to add a bolded "write more here" section to prompt me to provide and write documentation.
- Do not use language like "simple", "obvious", "trivial", or "easy".

## Workflow

### Build & Test Always
After making changes:
```bash
cargo build
cargo test
```

Do not move on until both pass.

### Clippy-Driven Development
Lean heavily into clippy. Run frequently:
```bash
cargo clippy -- -W clippy::pedantic
```

Fix warnings as they come up, not later. Clippy suggestions often lead to more idiomatic code.
In the case of a very noisy clippy that is difficult to address, prompt the user to take a look.

### Formatting
Always format upon completion of a task:
```bash
cargo fmt
```

## Testing

- Unit tests go in the same file as the code (`#[cfg(test)]`)
- Integration tests in `tests/` directory
- Use `tempfile` crate for tests that need a database
- Test the happy path first, then edge cases
