## Logging Levels

This CLI serves both humans and agents (via `--format json`). Follow these rules:

- `error!`: Unrecoverable failures
- `warn!`: Recoverable issues the user should know about (e.g. "not logged in, feed may be limited")
- `info!`: Only for interactive prompts the user needs to see in real-time (e.g. "Please scan the QR code"). Never for data that is already in the structured output.
- `debug!`: Everything else: progress messages, internal state, step completions

### Key Principle

If a piece of information is already in the command's result struct (e.g. `ExploreResult`, `StatusResult`), it must NOT be logged at info level. Duplicate it at `debug!` at most. The result goes to stdout via the `output` module; logs go to stderr via tracing.

## Structured Output

- All commands return a serializable result struct (`Serialize` + `Display`)
- `Display` impl provides human-friendly text; `Serialize` provides JSON for agents
- Use `output.result(&value)` to write to stdout based on `--format` flag
- Never mix result data into tracing/logs

## Command Loop Pattern

All 4 commands (`explore`, `search`, `creator`, `note`) follow the same skeleton:

```
check CancellationToken → check StopCondition → extract → process → interact → check StopCondition → stagnation → advance → delay
```

### Shared modules (`src/commands/`)

- `support.rs`: `StopCondition` (OR: max_items or duration), `StagnationTracker` (two profiles), `process_card_batch`, `card_unique_key`, `matches_keywords`
- `interact.rs`: `open_note`, `browse_note`, `close_note_detail` — used by explore, search, creator when `--interact` is set

### Stagnation profiles

- `for_feed()` (explore/search/creator): Continue → Sprint (5 rapid scrolls at stagnant=5) → GiveUp (if still stuck after sprint)
- `for_comments()` (note): Continue → Escalate (larger scroll at stagnant=5) → Sprint (10 scrolls at stagnant=20) → reset and repeat
