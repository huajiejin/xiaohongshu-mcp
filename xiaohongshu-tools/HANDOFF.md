# HANDOFF.md — xiaohongshu-tools Development Context

> Last updated after completing Phase 1 Batch 1 & 2 (anti-detection foundation + retry)

## What Was Done

### Batch 1: Chrome Args Hardening + Proxy + Viewport Randomization

**`src/browser.rs`** — Major rewrite:
- New `BrowserOptions` struct (`headless`, `proxy`) replaces raw `headless: bool` param
- Added `--disable-blink-features=AutomationControlled` (critical anti-detection flag)
- Fixed `lang` from `en_US` -> `zh-CN` (it's a Chinese platform)
- Added `--disable-infobars`, `--disable-notifications`
- Random viewport selection from 6 common resolutions on each launch
- Proxy support: `--proxy` CLI flag or `XHS_PROXY` env var, passed as `--proxy-server` Chrome arg
- `mask_proxy_credentials()` — masks user:pass in log output

**`src/auth.rs`** — Updated to use `&BrowserOptions` instead of `headless: bool`

**`src/bin/xiaohongshu-cli/main.rs`** — Added `--proxy` global flag, builds `BrowserOptions`

### Batch 2: Retry with Jitter

**`src/retry.rs`** — New module:
- `RetryConfig` struct with `max_retries`, `base_delay`, `max_jitter`
- `retry()` async function with exponential backoff + random jitter
- Default config: 3 retries, 500ms base, 200ms max jitter
- 3 unit tests (all passing)

**`Cargo.toml`** — Added `rand = "0.9"` dependency

**`src/lib.rs`** — Added `pub mod retry;`

## Current Architecture

```
src/
├── lib.rs              # Module declarations: auth, browser, cookies, retry
├── auth.rs             # Login (QR scan) + check_status, uses BrowserOptions
├── browser.rs          # Browser launch (chromiumoxide), stealth, cookies injection, proxy, viewport
├── cookies.rs          # Cookie persist/load/delete (JSON file at ~/.config/xiaohongshu/)
├── retry.rs            # Generic retry with exponential backoff + jitter
└── bin/
    └── xiaohongshu-cli/
        └── main.rs     # CLI entry point (clap), --headless --proxy flags
```

## CLI Usage (current)

```bash
xhs auth login                          # Opens browser, scan QR to login
xhs auth status                         # Check if cookies are still valid
xhs --proxy socks5://host:port auth login  # Use proxy
xhs --headless auth status              # Headless status check
XHS_PROXY=http://proxy:8080 xhs auth login  # Proxy via env var
```

## What's Next (PLAN.md Reference)

### Batch 3: Human Behavior Simulation (next priority, 2-3 sessions)

This is the **biggest anti-detection gap** vs the competitor. Create `src/human.rs`:

1. **3a — Core delays + scroll simulation** (session 1):
   - `BehaviorConfig` with 7 delay ranges (human_delay, reaction_time, hover_time, read_time, short_read, scroll_wait, post_scroll)
   - `HumanBehavior` struct with `ThreadRng`
   - `random_delay()` — Gaussian-distributed random sleep
   - `scroll_page()` — variable delta based on viewport + noise, stagnation detection
   - Three scroll speed modes: slow (1200ms), normal (600ms), fast (300ms)

2. **3b — Mouse movement + click simulation** (session 2):
   - `human_click()` — move mouse to element center, hover delay, then click
   - `interact_with_element()` — scroll into view -> reaction time -> hover -> click -> read
   - `stagnation_recovery()` — "big sprint" when scroll stops progressing

3. **3c — Integration** (session 3):
   - Integrate `HumanBehavior` into auth flow
   - Use `retry::retry()` wrapper for page operations in auth.rs
   - Prepare for Phase 2 commands (search, feed detail, etc.)

### Phase 2: Core Features

After Batch 3, implement feature parity with the competitor:
- `xhs search <query>` — search with filters
- `xhs feed <note_id>` — note detail with comments
- `xhs explore` — browse homepage feed
- `xhs profile <user_id>` — user profile
- `xhs like <note_id>` / `xhs favorite <note_id>`
- `xhs comment <note_id> <text>`

All will need `src/extractor.rs` for reading `window.__INITIAL_STATE__` via JS eval.

### Phase 3: Competitive Edge

- Multi-account sessions (`--profile` flag)
- Adaptive rate limiting
- Cookie encryption at rest
- Output formats (JSON/CSV/table)
- Daemon mode

## Key Design Decisions

1. **Browser-first approach**: All XHS interaction goes through a real Chromium browser (chromiumoxide + CDP). No direct HTTP API calls. This means `x-s`/`x-t` signing is handled by the website's own JS — we never need to reverse-engineer it.

2. **`BrowserOptions` is the central config**: Every function that needs a browser takes `&BrowserOptions`. This makes it easy to add new options (user-agent, viewport override, etc.) without changing signatures everywhere.

3. **`retry::retry()` is a generic utility**: It wraps any async operation. Will be used extensively when we add commands that do page navigation, scrolling, and DOM queries.

4. **Competitor reference**: The Go competitor (`xiaohongshu-mcp/`) is in the same repo. Key file to reference for behavioral patterns: `xiaohongshu-mcp/xiaohongshu/feed_detail.go` — has the most sophisticated human-like behavior code.

## Dependencies

```toml
anyhow = "1.0.102"           # Error handling
chromiumoxide = "0.9.1"      # CDP browser automation
clap = { version = "4.6.0", features = ["derive"] }  # CLI
dirs = "6.0.0"               # Config directory
futures = "0.3.32"           # Stream handling for CDP handler
rand = "0.9"                 # Random delays, viewport selection, jitter
serde = { version = "1.0.228" }  # Cookie serialization
serde_json = "1.0.149"       # JSON
thiserror = "2.0.18"         # Custom error types
tokio = { version = "1.50.0", features = ["full"] }  # Async runtime
tracing = "0.1.44"           # Logging
tracing-subscriber = "0.3.23"  # Log output
```

## Build & Test

```bash
cd xiaohongshu-tools
cargo build          # Build
cargo test           # Run tests (retry module has 3 tests)
cargo run -- --help  # CLI help
RUST_LOG=debug cargo run -- auth status  # Debug logging
```
