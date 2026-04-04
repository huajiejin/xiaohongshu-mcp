# HANDOFF.md — xiaohongshu-tools Development Context

> Last updated after `xhs note` command implementation

### New Findings (Before Refactor Work)

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

### Batch 11: Search Command + Creator Command + Creator Token Flow

**`src/commands/search.rs`** — Search command:
- Full search with filter support: `--sort_by`, `--note_type`, `--publish_time`, `--search_scope`, `--location`
- Uses shared `extract_feed_cards_with_fallback(..., FeedStateRoot::Search)`
- Filter UI interaction via hover-to-reveal panel, click selectors derived from enum-to-grid mapping
- `ApiResponseWatcher` for reliable filter-change detection
- `SearchResult` with `collected_at` timestamp

**`src/commands/creator.rs`** — Creator command:
- Accepts a full profile URL (e.g. `https://www.xiaohongshu.com/user/profile/<id>?xsec_token=...&xsec_source=pc_feed`)
- Parses user_id from URL via `parse_creator_url()` for the result struct
- Extracts user info (nickname, red_id, desc, ip_location, gender, avatar) from `__INITIAL_STATE__.user.userPageData.basicInfo`
- Extracts interactions (followers, following, posts count) from `__INITIAL_STATE__.user.userPageData.interactions`
- Collects posts via `FeedStateRoot::UserProfile` → `__INITIAL_STATE__.user.notes`
- `CreatorResult` includes `user_info`, `interactions`, `posts`, `collected_at`

**`src/feed_extract.rs`** — Creator token extraction:
- Added `creator_xsec_token` field to `FeedCard` (extracted from `noteCard.user.xsecToken`)
- Added `FeedStateRoot::UserProfile` variant → reads `user_notes` from initial state
- Added `parse_creator_url()` — extracts `(user_id, xsec_token)` from a full profile URL
- Added `creator_profile_url()` on `FeedCard` — builds full profile URL from `creator_id` + `creator_xsec_token`
- Extractor JS now also extracts `user_notes` and `user_data` from `__INITIAL_STATE__`

**`src/extractor.rs`** — Updated JS extraction:
- Added `flatten()` helper for nested arrays in `user.notes`
- Now extracts `user_notes` and `user_data` alongside `feed_feeds` and `search_feeds`

**`src/commands/explore.rs`** — Display updated:
- Post lines now show `creator`, `creator_id`, `profile` (full URL) instead of separate token fields
- Profile URL is copy-pasteable for direct use with `xhs creator`

**`src/commands/search.rs`** — Display updated:
- Post lines now show `creator_id` and `profile` (full URL) for creator exploration

**`locales/en.yml` + `locales/zh-CN.yml`** — New keys:
- `cli.creator_about`, `cli.url_help` (replaced `user_id_help`/`xsec_token_help`)
- `creator.matched_posts`, `creator.post_line`, `creator.wait_initial_state_timeout`, `creator.invalid_url`
- `explore.post_line` and `search.post_line` updated to show `profile: <url>` instead of `token: <xsec_token>`

**`src/lib.rs`** — Updated i18n helpers:
- Replaced `cli_user_id_help()` / `cli_xsec_token_help()` with `cli_url_help()`

**`Cargo.toml`** — Added `chrono` with serde feature for timestamp serialization

### Batch 12: author → creator rename

Renamed all `author` references to `creator` across the entire codebase:

**`src/feed_extract.rs`:**
- `FeedCard` fields: `author_name` → `creator_name`, `author_id` → `creator_id`, `author_xsec_token` → `creator_xsec_token`
- Method: `author_profile_url()` → `creator_profile_url()`
- Function: `parse_profile_url()` → `parse_creator_url()`
- All test references updated

**`src/commands/creator.rs`** (renamed from `profile.rs`):
- `ProfileOptions` → `CreatorOptions`
- `ProfileResult` → `CreatorResult`
- Locale keys: `profile.*` → `creator.*`

**`src/commands/explore.rs`** + **`src/commands/search.rs`:**
- Display code uses `creator`, `creator_id`, `profile_url` variable names

**`src/bin/xiaohongshu-cli/main.rs`:**
- `Commands::Profile` → `Commands::Creator`
- Subcommand name: `"profile"` → `"creator"`
- Import: `profile` → `creator`

**`src/lib.rs`:**
- `cli_profile_about()` → `cli_creator_about()`

**`locales/en.yml`:**
- `cli.profile_about` → `cli.creator_about`
- `explore.post_line`: `author` → `creator`, `author_id` → `creator_id`
- `search.post_line`: `author` → `creator`, `author_id` → `creator_id`
- Section: `profile:` → `creator:`

**`locales/zh-CN.yml`:**
- Same structural changes, `作者` → `创作者`

### Batch 13: Module Reorganization

**Full reorganization of `src/` into module folders:**

- `browser/` — `launch.rs` (was browser.rs), `cookies.rs`, `human.rs`
- `auth/` — `flow.rs` (was auth.rs), `login_image.rs`
- `extract/` — `note.rs` (merged `extractor.rs` + `note_extract.rs`)
- `shared/` — `i18n.rs` (extracted from lib.rs inline module), `output.rs`, `retry.rs`, `utils.rs`
- `commands/` — unchanged structure, updated imports
- All `mod.rs` files are re-export only (no business logic)
- `lib.rs` reduced to: `rust_i18n::i18n!()` macro + 5 `pub mod` declarations + `pub use t!`
- Fixed 6 clippy nursery warnings (redundant clones, option_if_let_else, or_fun_call)
- All 10 tests pass, zero clippy warnings

### Batch 14: `xhs note` Command (Note Detail + Comments)

**`src/extract/note.rs`** — New structs and extraction:
- `NoteImage` struct: `url`, `width`, `height`, `live_photo`
- `CommentRaw` struct: raw comment from `__INITIAL_STATE__` with recursive `sub_comments`
- `Comment` struct: normalized (parsed counts, formatted timestamps) with `from_raw()`
- `NoteDetailRaw` struct: raw note detail from `__INITIAL_STATE__.note.noteDetailMap[note_id]`
- `NoteDetail` struct: normalized detail with `from_raw()`
- `NoteResult` struct: command result wrapper with `Display` impl for text output
- `extract_note_detail_map()`: new JS extraction for `window.__INITIAL_STATE__.note.noteDetailMap`
- `parse_note_detail_raw()`: parses JSON into `NoteDetailRaw` (note fields + comments + images)
- `parse_image_list()`: extracts images from `imageList` array (prefers `urlDefault` → `urlPre`)
- `parse_comment_raw()`: parses recursive comment tree from JSON
- `extract_video_url_from_dom()`: queries `<video>` element for `src` — Rust advantage over Go
- `check_note_page_accessible()`: detects deleted/private/violation/blocked notes via DOM text
- `parse_timestamp_ms()`: converts Unix ms timestamps to ISO datetime strings
- `str_field()`: helper for extracting optional non-empty string fields from JSON

**`src/commands/note.rs`** — New command (290 lines):
- `NoteOptions`: `url`, `max_comments` (default 20), `max_replies` (default 10), `scroll_speed`, `duration` (default 120s)
- `run()`: parse URL → navigate → check accessibility → extract detail → extract video URL → optional comment loading → return `NoteResult`
- Comment loading loop (`load_comments()`):
  - Scrolls to comments area (`.comments-container`)
  - Detects no-comments marker ("荒地")
  - Detects end-of-comments marker ("THE END")
  - Clicks "show more" buttons for sub-replies (respects `max_replies` threshold)
  - Stagnation detection with progressive escalation (large scroll at 5, big sprint at 20)
  - Re-reads `__INITIAL_STATE__` after scrolling for final comment state
- DOM helper functions: `count_dom_comments`, `check_end_container`, `check_no_comments`, `scroll_to_comments_area`, `scroll_comments`
- `wait_note_page()`: polls for `noteDetailMap` availability in `__INITIAL_STATE__`

**`src/extract/mod.rs`** — Updated exports for new types and functions

**`src/commands/mod.rs`** — Added `pub mod note`

**`src/bin/xiaohongshu-cli/main.rs`** — Added `Note` subcommand:
- `xhs note <url>` with `--max_comments`, `--max_replies`, `--scroll_speed`, `--duration`
- CLI help via i18n helpers

**`src/shared/i18n.rs`** — Added helpers: `cli_note_about`, `cli_note_url_help`, `cli_max_comments_help`, `cli_max_replies_help`

**`locales/en.yml`** + **`locales/zh-CN.yml`** — New `note:` section with 13 keys for Display output and error messages

**Also fixed** pre-existing clippy warnings: `or_fun_call` in creator.rs, `use_self` + `missing_const_for_fn` in ExtractionRoot

All 26 tests pass, zero clippy warnings (including `-W clippy::nursery`).

### Remaining follow-up work (next session)

- Add optional count normalization helpers (e.g. `1.2w` -> numeric) if downstream consumers need numeric sorting.

### Go Competitor Gap Analysis (all 13 MCP tools audited)

Go competitor has **13 registered MCP tools** (`mcp_server.go`). Rust CLI currently covers 7 (auth login/logout/status, explore, search, creator, note).

**Already implemented in Rust (4/13):**

| Go Tool | Rust CLI | Status |
|---|---|---|
| `check_login_status` | `xhs auth status` | Done |
| `get_login_qrcode` | `xhs auth login` | Done |
| `delete_cookies` | `xhs auth logout` | Done |
| `list_feeds` | `xhs explore` | Done |
| `search_feeds` | `xhs search` | Done |
| `user_profile` | `xhs creator` | Done |
| `get_feed_detail` | `xhs note` | Done |

**Missing — ranked by priority:**

| # | Go Tool | Feature | Impact | Complexity | Build Order |
|---|---|---|---|---|---|
| 1 | `like_feed` | Like/unlike a note (idempotent) | MEDIUM | Low | **1st** |
| 2 | `favorite_feed` | Favorite/unfavorite a note (idempotent) | MEDIUM | Low | **1st** |
| 3 | `post_comment_to_feed` | Post top-level comment on a note | MEDIUM | Low | **2nd** |
| 4 | `reply_comment_in_feed` | Reply to an existing comment | MEDIUM | Low | **2nd** |
| 5 | `publish_content` | Publish image+text note (tags, scheduling, visibility, product binding) | HIGH | High | **3rd** |
| 6 | `publish_with_video` | Publish video note (tags, scheduling, visibility, product binding) | HIGH | High | **3rd** |

**Go also has a REST-only `/api/v1/user/me` endpoint** (not an MCP tool) for fetching the logged-in user's own profile. Low priority.

### Recommended build order

1. ~~**`xhs note <url>`** — done (Batch 14)~~
2. **`xhs like` + `xhs favorite`** — simple click actions, reuse navigation from note. Go makes these idempotent (skip if already in desired state).
3. **`xhs comment` + `xhs reply`** — text input interactions on note detail page.
4. **`xhs publish` (image+text) + `xhs publish` (video)** — most complex (file uploads, form filling, scheduling).

## Current Architecture

```
src/
├── lib.rs                       # rust_i18n init macro + pub mod declarations + re-export t!
├── browser/
│   ├── mod.rs                   # Re-exports: BrowserOptions, launch, extract_cookies, etc.
│   ├── launch.rs                # Browser launch (chromiumoxide), stealth, cookies injection, proxy, viewport
│   ├── cookies.rs               # Cookie persist/load/delete (JSON file at config dir)
│   └── human.rs                 # Human behavior simulation (delays, scrolling, clicking)
├── auth/
│   ├── mod.rs                   # Re-exports: login, logout, check_status, result structs
│   ├── flow.rs                  # Auth flows: login (QR scan), logout, status check
│   └── login_image.rs           # QR code image extraction and display
├── extract/
│   ├── mod.rs                   # Re-exports: NoteCard, NoteDetail, ExtractionRoot, extract_* functions
│   └── note.rs                  # __INITIAL_STATE__ JS eval + note card/detail extraction + DOM fallback
├── shared/
│   ├── mod.rs                   # Re-exports: Format, Output, RetryConfig, etc.
│   ├── i18n.rs                  # Locale detection + CLI help string helpers
│   ├── output.rs                # Structured output (text/json) via --format flag
│   ├── retry.rs                 # Generic retry with exponential backoff + jitter
│   └── utils.rs                 # Shared utilities (polling, data URL decode, file open, API watcher)
├── commands/
│   ├── mod.rs                   # Re-exports command submodules
│   ├── explore.rs               # Explore discover feed with filtering and interaction
│   ├── search.rs                # Search with filters (sort, type, time, scope, location)
│   ├── creator.rs               # Creator profile exploration (accepts profile URL)
│   └── note.rs                  # Note detail with comments (scroll loading, sub-reply expansion)
└── bin/
    └── xiaohongshu-cli/
        └── main.rs              # CLI entry point (clap), --format, --headless, --proxy
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

xhs search "keyword"                     # Search posts
xhs search "keyword" --sort_by latest    # Sort: general, latest, most_liked, most_commented, most_collected
xhs search "keyword" --note_type video   # Type: all, video, normal
xhs search "keyword" --max_posts 20      # Limit results

xhs creator "https://www.xiaohongshu.com/user/profile/<user_id>?xsec_token=<token>&xsec_source=pc_feed"
xhs creator "<url>" --max_posts 10       # Limit creator posts
xhs creator "<url>" --duration 60        # Time-limited collection

xhs note "https://www.xiaohongshu.com/explore/<note_id>?xsec_token=<token>&xsec_source=pc_feed"
xhs note "<url>" --max_comments 50       # Load up to 50 top-level comments
xhs note "<url>" --max_replies 5         # Expand sub-replies up to 5 per comment
xhs note "<url>" --max_comments 0        # Skip comment scrolling (return initial batch only)
xhs note "<url>" --scroll_speed slow     # Scroll speed for comment loading
xhs note "<url>" --duration 60           # Max seconds to spend collecting

xhs --format json auth status           # JSON output for agents
xhs --format json explore --max_posts 5  # JSON explore results
xhs --format json search "cats"          # JSON search results
xhs --format json creator "<url>"        # JSON creator results
xhs --format json note "<url>"           # JSON note detail + comments
```

## What's Next (PLAN.md Reference)

### Phase 2: Core Features (in progress — 7/10 done)

Implement feature parity with the competitor:
- ~~`xhs search <query>` — search with filters~~ (done)
- ~~`xhs explore` — browse homepage feed~~ (done)
- ~~`xhs creator <url>` — creator profile~~ (done, accepts profile URL)
- ~~`xhs note <url>` — note detail with comments~~ (done, Batch 14)
- `xhs like <note_id>` / `xhs favorite <note_id>` **(next to build)**
- `xhs comment <note_id> <text>`
- `xhs reply <note_id> <comment_id> <text>`
- `xhs publish` — image+text and video note publishing

All use `src/extract/note.rs` for reading `window.__INITIAL_STATE__` via JS eval.

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
chrono = { version = "0.4", features = ["serde"] }  # Timestamps
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
cargo test           # Run tests (extract::note: 10, shared::parse: 11, shared::retry: 3, total: 26)
cargo fmt            # Format
cargo run -- --help  # CLI help
RUST_LOG=debug cargo run -- auth status  # Debug logging
```
