# HANDOFF.md — xiaohongshu-tools Development Context

> Last updated after completing Phase 1 + output module and best practices pass

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

### Batch 3: Human Behavior Simulation

**`src/human.rs`** — New module (249 lines):
- `BehaviorConfig` with 7 configurable delay ranges (human_delay, reaction_time, hover_time, etc.)
- `HumanBehavior` struct with `StdRng` (Send-safe)
- `random_delay()` — Gaussian-distributed random sleep
- `scroll_page()` — variable delta with noise, stagnation detection, big sprint recovery
- `human_click()` — move mouse to element, hover delay, click
- `interact_with_element()` — scroll into view → reaction → hover → click → read
- Three scroll speed modes: `Slow`, `Normal`, `Fast`
- `ScrollSpeed` enum with `FromStr` parsing

### Batch 4: Login Image + Shared Utils

**`src/login_image.rs`** — Extract QR code image from login page:
- Fetches `.qrcode-img` src, decodes data URL via `base64`
- Saves to temp file, opens with system viewer (`open`/`xdg-open`)
- Falls back gracefully if image fetch fails

**`src/utils.rs`** — Shared utilities:
- `poll_until()` — generic async polling with timeout + interval
- `decode_data_url()` — parse `data:image/...;base64,...` into bytes
- `open_file()` — cross-platform file opener

**`Cargo.toml`** — Added `base64 = "0.22"` dependency

### Batch 5: Browse Command + Extractor

**`src/extractor.rs`** — New module:
- `extract_initial_state()` — reads `window.__INITIAL_STATE__` via JS eval
- `extract_feed_items()` — parses feed items from initial state JSON
- `extract_note_detail()` — extracts note content, comments, user info

**`src/commands/browse.rs`** — Browse explore feed command:
- `BrowseOptions` with keywords, exclude, max_posts, scroll_speed, interact, duration
- Scrolls explore feed, filters posts by keywords/exclude
- Optional `--interact` mode: click into posts, scroll comments, close
- `--duration` auto-exit timer
- `--max_posts` limit on matched posts

**`src/commands/mod.rs`** — Commands module root

### Batch 6: Auth Logout

**`src/bin/xiaohongshu-cli/main.rs`** — Added `auth logout` subcommand:
- Calls `cookies::delete_cookies()` to remove cookie file
- No browser launch needed

### Batch 7: Structured Output Module

**`src/output.rs`** — New module:
- `Format` enum (Text/Json) with `FromStr` parsing
- `Output` struct with `result()` method — writes to stdout based on format
- `--format` global CLI flag (default: text)

**`src/auth.rs`** — Commands now return serializable result structs:
- `LoginResult { logged_in: bool }`
- `StatusResult { logged_in: bool }`
- `LogoutResult { logged_out: bool }`

**`src/commands/browse.rs`** — Returns `BrowseResult { total, posts: Vec<PostItem>, duration_secs }`

**`src/bin/xiaohongshu-cli/main.rs`** — All commands route results through `Output::result()`

### Batch 8: Rust Best Practices Pass

**Multiple files** — Clippy compliance (0 warnings with `-W clippy::nursery`):
- `browser.rs`: scoped `rng` to avoid `!Send` across `.await`, `.get()` over `[]`, removed redundant clone
- `human.rs`: `ThreadRng` → `StdRng` (Send-safe), `const fn` where possible, derived `Eq`
- `browse.rs`: scoped local `rng` to avoid `!Send` across `.await`
- `cookies.rs`: `unwrap_or` → `unwrap_or_else`
- All non-test `unwrap()` eliminated

### Batch 9: Logging Level Cleanup

**Multiple files** — Separated result data from logging:
- 12 statements downgraded (`info!` → `debug!`, 1 `warn!` → `debug!`)
- `info!` only for interactive prompts (QR scan prompts, auto-open fallback)
- `warn!` only for actionable issues (not logged in, browse failures)
- **`AGENTS.md`** created with logging and output conventions

## Current Architecture

```
src/
├── lib.rs              # Module declarations
├── auth.rs             # Login (QR scan) + check_status, returns structured results
├── browser.rs          # Browser launch (chromiumoxide), stealth, cookies injection, proxy, viewport
├── cookies.rs          # Cookie persist/load/delete (JSON file at config dir)
├── output.rs           # Structured output (text/json) via --format flag
├── retry.rs            # Generic retry with exponential backoff + jitter
├── human.rs            # Human behavior simulation (delays, scrolling, clicking)
├── extractor.rs        # Extract data from window.__INITIAL_STATE__ via JS eval
├── login_image.rs      # QR code image extraction and display
├── utils.rs            # Shared utilities (polling, data URL decode, file open)
├── commands/
│   ├── mod.rs
│   └── browse.rs       # Browse explore feed with filtering and interaction
└── bin/
    └── xiaohongshu-cli/
        └── main.rs     # CLI entry point (clap), --format, --headless, --proxy
```

## CLI Usage (current)

```bash
xhs auth login                          # Opens browser, scan QR to login
xhs auth logout                         # Remove saved cookies
xhs auth status                         # Check if cookies are still valid
xhs --proxy socks5://host:port auth login  # Use proxy
xhs --headless auth status              # Headless status check
XHS_PROXY=http://proxy:8080 xhs auth login  # Proxy via env var

xhs browse                              # Browse explore feed
xhs browse --keywords cat,dog           # Only interact with matching posts
xhs browse --exclude ad,sponsored       # Skip matching posts
xhs browse --max_posts 10               # Stop after 10 matched posts
xhs browse --scroll_speed slow          # Scroll speed: slow, normal, fast
xhs browse --interact                   # Click into posts and scroll comments
xhs browse --duration 300               # Auto-exit after 5 minutes

xhs --format json auth status           # JSON output for agents
xhs --format json browse --max_posts 5  # JSON browse results
```

## What's Next (PLAN.md Reference)

### Phase 2: Core Features

Implement feature parity with the competitor:
- `xhs search <query>` — search with filters
- `xhs feed <note_id>` — note detail with comments
- `xhs explore` — browse homepage feed (partially done via `browse` command)
- `xhs profile <user_id>` — user profile
- `xhs like <note_id>` / `xhs favorite <note_id>`
- `xhs comment <note_id> <text>`

All will use `src/extractor.rs` for reading `window.__INITIAL_STATE__` via JS eval.

### Phase 3: Competitive Edge

- Multi-account sessions (`--profile` flag)
- Adaptive rate limiting
- Cookie encryption at rest
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
serde = "1.0.228"            # Cookie + state serialization
serde_json = "1.0.149"       # JSON
thiserror = "2.0.18"         # Custom error types
tokio = { version = "1.50.0", features = ["full"] }  # Async runtime
tracing = "0.1.44"           # Logging
tracing-subscriber = { version = "0.3.23", features = ["env-filter"] }  # Log output
base64 = "0.22"              # Data URL decoding for login image
```

## Build & Test

```bash
cd xiaohongshu-tools
cargo build          # Build
cargo test           # Run tests (retry module has 3 tests)
cargo run -- --help  # CLI help
RUST_LOG=debug cargo run -- auth status  # Debug logging
```
