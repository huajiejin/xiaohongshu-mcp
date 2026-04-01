## Logging Levels

This CLI serves both humans and agents (via `--format json`). Follow these rules:

- **`error!`** — Unrecoverable failures
- **`warn!`** — Recoverable issues the user should know about (e.g. "not logged in, feed may be limited")
- **`info!`** — Only for interactive prompts the user needs to see in real-time (e.g. "Please scan the QR code"). Never for data that is already in the structured output.
- **`debug!`** — Everything else: progress messages, internal state, step completions

### Key Principle

If a piece of information is already in the command's result struct (e.g. `BrowseResult`, `StatusResult`), it must NOT be logged at info level. Duplicate it at `debug!` at most. The result goes to stdout via the `output` module; logs go to stderr via tracing.

## Structured Output

- All commands return a serializable result struct (`Serialize` + `Display`)
- `Display` impl provides human-friendly text; `Serialize` provides JSON for agents
- Use `output.result(&value)` to write to stdout based on `--format` flag
- Never mix result data into tracing/logs
