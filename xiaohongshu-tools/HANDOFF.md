# HANDOFF.md — xiaohongshu-tools Development Context

> Last updated after completing Phase 1 + output module and best practices pass

## New Findings (Before Refactor Work)

### Explore/Browse and Search extraction gaps

- Current Rust `explore` is DOM-only (`#exploreFeeds section` + `a.cover` + title span), so result fields are limited and selector changes can break extraction.
- Go competitor search uses `window.__INITIAL_STATE__.search.feeds` (with Vue proxy fallback `value` / `_value`) and only falls back to errors when state is unavailable.
- `explore` path data lives in `window.__INITIAL_STATE__.feed.feeds`, while `search` path data lives in `window.__INITIAL_STATE__.search.feeds` — shared extractor must support both roots.
- Existing Rust `extractor.rs` already has `extract_initial_state()`, so we can build a reusable feed extraction layer above it.

### Data model findings from real feed objects

- Stable follow-up action keys are available in feed-level fields: `id` and `xsecToken`.
- Useful note-level fields are available under `noteCard`: `type`, `displayTitle`, `user` (`userId`, `nickName`), `interactInfo` (`liked`, `likedCount`, optional `commentCount/sharedCount/collectedCount`), `cover`, optional `video.capa.duration`.
- Cover URL can be selected by preference order: `cover.urlDefault` -> `cover.urlPre` -> `cover.url` -> matching best candidate from `cover.infoList` (`WB_DFT` preferred, then `WB_PRV`, then first non-empty).
- Card href usually follows `/explore/{id}?xsec_token=...&xsec_source=...`; parsing href into structured parts is useful for consistent downstream tooling.

### Refactor direction agreed in this session

- Build a shared feed extraction module with strategy: `__INITIAL_STATE__` first, DOM card parsing fallback.
- Refactor `explore` to consume shared extractor and return richer structured post fields (not just title/href).
- Keep behavior simulation/interact flow, but decouple result extraction from brittle DOM-only fields.

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

**`src/commands/explore.rs`** — Explore discover feed command:
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

**`src/commands/explore.rs`** — Returns `ExploreResult { total, posts: Vec<FeedCard>, duration_secs }`

**`src/bin/xiaohongshu-cli/main.rs`** — All commands route results through `Output::result()`

### Batch 8: Rust Best Practices Pass

**Multiple files** — Clippy compliance (0 warnings with `-W clippy::nursery`):
- `browser.rs`: scoped `rng` to avoid `!Send` across `.await`, `.get()` over `[]`, removed redundant clone
- `human.rs`: `ThreadRng` → `StdRng` (Send-safe), `const fn` where possible, derived `Eq`
- `explore.rs`: scoped local `rng` to avoid `!Send` across `.await`
- `cookies.rs`: `unwrap_or` → `unwrap_or_else`
- All non-test `unwrap()` eliminated

### Batch 9: Logging Level Cleanup

**Multiple files** — Separated result data from logging:
- 12 statements downgraded (`info!` → `debug!`, 1 `warn!` → `debug!`)
- `info!` only for interactive prompts (QR scan prompts, auto-open fallback)
- `warn!` only for actionable issues (not logged in, explore failures)
- **`AGENTS.md`** created with logging and output conventions

### Batch 10: Shared Feed Extractor + Browse Refactor (INITIAL_STATE first)

**`src/feed_extract.rs`** — New shared module:
- Added shared extraction strategy: `__INITIAL_STATE__` first, DOM card parsing fallback
- Supports both roots via `FeedStateRoot`: `feed.feeds` (explore) and `search.feeds` (search)
- New normalized output struct `FeedCard` with key fields for follow-up actions:
  - `id`, `xsec_token`, `note_type`, `title`
  - `author_name`, `author_id`
  - `liked`, `liked_count`, `comment_count`, `shared_count`, `collected_count`
  - `cover_url`, `video_duration_secs`
- Added cover URL selection policy (best quality priority):
  `urlDefault` -> `urlPre` -> `url` -> `infoList(WB_DFT)` -> `infoList(WB_PRV)` -> first non-empty
- Added unit tests for href parsing, cover URL selection, and state item field extraction

**`src/commands/explore.rs`** — Refactored to use shared extractor:
- Replaced DOM-only extraction loop with shared `extract_feed_cards_with_fallback(..., FeedStateRoot::Explore)`
- `ExploreResult.posts` now returns richer structured card fields (instead of title/href only)
- Added consistent dedup key logic (`id` -> `href` -> `title`)
- Kept interaction mode (`--interact`) and updated click flow to locate cards by href/id robustly

**`src/lib.rs`**
- Exported new shared module: `pub mod feed_extract;`

### Remaining follow-up work (next session)

- Reuse `src/feed_extract.rs` in upcoming `search` command (`FeedStateRoot::Search`) for shared behavior.
- Add optional merge strategy when state data exists but misses fields (enrich from DOM instead of full fallback).
- Design and implement `search continue` pagination token contract for agent-friendly incremental loading.
- Add optional count normalization helpers (e.g. `1.2w` -> numeric) if downstream consumers need numeric sorting.

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
├── feed_extract.rs     # Shared feed card extraction (INITIAL_STATE first, DOM fallback)
├── login_image.rs      # QR code image extraction and display
├── utils.rs            # Shared utilities (polling, data URL decode, file open)
├── commands/
│   ├── mod.rs
│   └── explore.rs       # Explore discover feed with filtering and interaction
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

xhs explore                              # Explore discover feed
xhs explore --keywords cat,dog           # Only interact with matching posts
xhs explore --exclude ad,sponsored       # Skip matching posts
xhs explore --max_posts 10               # Stop after 10 matched posts
xhs explore --scroll_speed slow          # Scroll speed: slow, normal, fast
xhs explore --interact                   # Click into posts and scroll comments
xhs explore --duration 300               # Auto-exit after 5 minutes

xhs --format json auth status           # JSON output for agents
xhs --format json explore --max_posts 5  # JSON explore results
```

## What's Next (PLAN.md Reference)

### Phase 2: Core Features

Implement feature parity with the competitor:
- `xhs search <query>` — search with filters
- `xhs feed <note_id>` — note detail with comments
- `xhs explore` — browse homepage feed (done via `explore` command)
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
