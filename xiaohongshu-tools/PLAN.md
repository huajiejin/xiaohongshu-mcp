# xiaohongshu-tools: Anti-Detection & Competitive Analysis Plan

## 1. Current State Assessment

### Our Tool (xiaohongshu-tools, Rust)
- **Architecture**: Rust CLI using `chromiumoxide` (CDP) to control real Chromium
- **Features**: 2 commands only - `xhs auth login`, `xhs auth status`
- **Anti-detection**: Only `enable_stealth_mode()` (patches `navigator.webdriver` etc.)
- **Proxy**: None
- **Retry**: None
- **Behavioral simulation**: None
- **Cookie management**: JSON file at `~/.config/xiaohongshu/cookies.json`

### Competitor (xiaohongshu-mcp, Go)
- **Architecture**: Go MCP server using `go-rod` to control real Chrome
- **Features**: 13 MCP tools (search, feeds, publish, comments, likes, favorites, profiles, video)
- **Anti-detection**: `go-rod/stealth` + comprehensive human-like behavior
- **Proxy**: HTTP/HTTPS/SOCKS5 via `XHS_PROXY` env var
- **Retry**: `retry-go` with jitter, up to 3 retries per operation
- **Behavioral simulation**: 7 delay configs, mouse movement, scroll simulation, stagnation detection
- **Cookie management**: JSON file, configurable via `COOKIES_PATH`
- **Distribution**: Docker, docker-compose, MCP protocol

### Anti-Detection Score

| Measure | xiaohongshu-mcp | xiaohongshu-tools | Gap |
|---|:---:|:---:|:---:|
| Stealth mode | YES | YES | - |
| Human-like delays | YES (7 types) | NO | **Critical** |
| Scroll simulation | YES (3 speeds) | NO | **Critical** |
| Mouse movement | YES | NO | **High** |
| Retry with jitter | YES | NO | **High** |
| Proxy support | YES | NO | **High** |
| Chrome arg hardening | Partial | Partial | Medium |
| TLS fingerprint | Implicit (Chrome) | Implicit (Chrome) | - |
| API signing bypass | Browser-level | Browser-level | - |
| Rate limiting | No | No | - |
| Multi-account | No | No | - |
| Cookie encryption | No | No | - |

**Verdict: We are significantly behind on anti-detection. The competitor's human-like behavioral simulation is our biggest gap.**

---

## 2. Phase 1: Anti-Detection Foundation

### 2.1 Human-Like Behavioral Simulation Module (`src/human.rs`)

**Why**: Xiaohongshu's anti-bot system tracks timing patterns, scroll behavior, and mouse movements. The competitor implements 7 different delay configurations and multi-speed scrolling. We need to match and exceed this.

**Implementation**:

```rust
// src/human.rs - Human behavior simulation

pub struct BehaviorConfig {
    pub human_delay: Range<Duration>,      // 300-700ms general action delay
    pub reaction_time: Range<Duration>,    // 300-800ms before reacting
    pub hover_time: Range<Duration>,       // 100-300ms hover before click
    pub read_time: Range<Duration>,        // 500-1200ms reading delay
    pub short_read: Range<Duration>,       // 600-1200ms quick read
    pub scroll_wait: Range<Duration>,      // 100-200ms between scrolls
    pub post_scroll: Range<Duration>,      // 300-500ms after scroll
}

pub enum ScrollSpeed { Slow, Normal, Fast }

pub struct HumanBehavior {
    config: BehaviorConfig,
    rng: ThreadRng,
}

impl HumanBehavior {
    pub fn new() -> Self;
    pub fn random_delay(&mut self, range: &Range<Duration>) -> Duration;

    // Scrolling with variable delta, noise, and stagnation detection
    pub async fn scroll_page(&self, page: &Page, speed: ScrollSpeed) -> Result<ScrollResult>;

    // Move mouse to element, hover, then click
    pub async fn human_click(&self, page: &Page, selector: &str) -> Result<()>;

    // Multi-step: scroll into view -> wait -> hover -> click -> read
    pub async fn interact_with_element(&self, page: &Page, selector: &str) -> Result<()>;

    // Simulate reading content (variable delay based on content length)
    pub async fn simulate_reading(&self, content_length: usize) -> Result<()>;

    // "Big sprint" when scroll stagnation detected
    pub async fn stagnation_recovery(&self, page: &Page) -> Result<()>;
}
```

**Key behaviors**:
- Randomized inter-action delays (Gaussian distribution, not uniform)
- Variable scroll delta based on viewport height with noise term
- Mouse movement to element center before clicking
- Hover delay before click (mimics finding the right spot)
- Stagnation detection: if scroll delta < threshold for N consecutive scrolls, do a "big sprint"
- Three scroll speed modes: slow (1200ms+rand), normal (600ms+rand), fast (300ms+rand)
- Content-length-aware reading time (longer content = longer read)

**Files**: `src/human.rs` (new), `src/lib.rs` (add `pub mod human;`)

### 2.2 Proxy Support (`src/browser.rs`)

**Why**: IP bans are the most common anti-bot measure. The competitor supports HTTP/HTTPS/SOCKS5.

**Implementation**:

```rust
pub struct BrowserOptions {
    pub headless: bool,
    pub proxy: Option<ProxyConfig>,
}

pub struct ProxyConfig {
    pub url: String,  // e.g., "socks5://user:pass@host:port"
}

// Read from --proxy CLI flag or XHS_PROXY env var
// Pass as --proxy-server Chrome arg
// Mask credentials in logs
```

**Files**: `src/browser.rs`, `src/bin/xiaohongshu-cli/main.rs` (add `--proxy` flag)

### 2.3 Retry with Jitter (`src/retry.rs`)

**Why**: Network operations and page loads fail. The competitor retries all operations up to 3 times with jitter.

**Implementation**:

```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_jitter: Duration,
}

pub async fn retry_with_jitter<F, Fut, T>(
    config: &RetryConfig,
    operation: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    // Exponential backoff: base_delay * 2^attempt + random(0..max_jitter)
}
```

**Files**: `src/retry.rs` (new), `src/lib.rs` (add `pub mod retry;`)

### 2.4 Chrome Args Hardening (`src/browser.rs`)

**Why**: Missing critical anti-detection flags. `lang` is set to `en_US` which is wrong for a Chinese platform.

**Changes**:
```rust
// ADD:
"--disable-blink-features=AutomationControlled"

// FIX: Change lang from "en_US" to "zh-CN"
("--lang", &["zh-CN"][..])

// ADD:
"--disable-infobars"
"--disable-notifications"
// Randomize window size between common resolutions
```

**Files**: `src/browser.rs`

### 2.5 Viewport & Fingerprint Randomization

**Why**: Identical viewport/fingerprint every session is detectable.

```rust
const COMMON_VIEWPORTS: [(u32, u32); 6] = [
    (1920, 1080), (1366, 768), (1536, 864),
    (1440, 900), (1280, 720), (1600, 900),
];
// Pick random viewport on browser launch
// Set via --window-size Chrome arg
```

---

## 3. Phase 2: Core Features (Parity)

### 3.1 Search Feeds (`src/commands/search.rs`)

**Command**: `xhs search <query> [--sort general/latest] [--type all/video/image] [--time-limit no_limit/one_day/one_week]`

**Flow**:
1. Create browser with stealth + cookies
2. Navigate to `https://www.xiaohongshu.com/search_result?keyword=...&type=...`
3. Human-like scroll to load results
4. Extract `window.__INITIAL_STATE__` via JS eval
5. Parse and output results as JSON/table

### 3.2 Feed Detail (`src/commands/feed.rs`)

**Command**: `xhs feed <note_id> [--comments] [--comment-count 20]`

**Flow**:
1. Navigate to note page
2. Human-like scroll + read simulation
3. Expand comments if requested (human click on "show more")
4. Extract `window.__INITIAL_STATE__`

### 3.3 User Profile (`src/commands/profile.rs`)

**Command**: `xhs profile <user_id>`

### 3.4 List Feeds (`src/commands/explore.rs`)

**Command**: `xhs explore [--category <cat>]`

### 3.5 Like / Favorite (`src/commands/like.rs`)

**Command**: `xhs like <note_id>` / `xhs favorite <note_id>`

### 3.6 Post Comment (`src/commands/comment.rs`)

**Command**: `xhs comment <note_id> <text>`

---

## 4. Phase 3: Competitive Edge (Win)

### 4.1 Multiple Account Sessions

**Why**: The competitor doesn't have this. Power users manage multiple accounts.

**Implementation**:
- `xhs auth login --profile work` -> saves to `~/.config/xiaohongshu/profiles/work/cookies.json`
- `xhs auth login --profile personal` -> saves to separate profile
- `xhs --profile work search ...` -> uses specific profile
- Default profile: `default`

### 4.2 Adaptive Rate Limiting

**Why**: Neither tool has this. If XHS starts throttling, we auto-adapt.

**Implementation**:
```rust
pub struct RateLimiter {
    min_interval: Duration,
    current_interval: Duration,
    max_interval: Duration,
    backoff_factor: f64,
    recovery_factor: f64,
    last_request: Instant,
}

// Detect throttle signals:
// - Page shows captcha
// - Page returns empty results unexpectedly
// - __INITIAL_STATE__ is empty
// - Page redirect to verification
```

### 4.3 Cookie Encryption at Rest

**Why**: Plaintext cookies.json is a security risk. Competitor doesn't encrypt either.

**Implementation**:
- AES-256-GCM encryption of cookies file
- Key derived from user passphrase via Argon2
- `xhs auth login --encrypt` prompts for passphrase
- Auto-detect: if file starts with `{`, it's plaintext; otherwise encrypted

### 4.4 Session Health Monitoring

**Why**: Users need to know if their session is still valid before running batch operations.

**Implementation**:
- `xhs auth status` returns detailed session info (expiry, last used, health score)
- Auto-check before any operation
- Warn and prompt for re-login if session is stale

### 4.5 Output Formats

**Command**: `xhs search ... --format json|csv|table|markdown`

**Why**: Competitor outputs raw JSON. Users want flexibility.

### 4.6 Daemon Mode

**Command**: `xhs daemon start` / `xhs daemon stop`

**Why**: Long-running background process for batch operations, scheduled tasks, webhooks.

---

## 5. Phase 4: Distribution

### 5.1 MCP Protocol Support

**Why**: The competitor's main distribution channel is MCP. We need to support it too.

**Implementation**: Add an MCP server mode that exposes all commands as MCP tools.

### 5.2 Homebrew Tap

```bash
brew install xiaohongshu-tools/tap/xhs
```

### 5.3 Docker Image

Multi-arch Docker image with Chromium pre-installed.

### 5.4 GitHub Actions CI

Auto-build releases for macOS (arm64/x86_64), Linux (arm64/x86_64), Windows.

---

## 6. File Structure Plan

```
xiaohongshu-tools/
├── Cargo.toml
├── PLAN.md
├── src/
│   ├── lib.rs
│   ├── auth.rs              # (existing, enhance)
│   ├── browser.rs           # (existing, enhance with proxy, hardened args)
│   ├── cookies.rs           # (existing, enhance with encryption)
│   ├── human.rs             # NEW: behavioral simulation
│   ├── retry.rs             # NEW: retry with jitter
│   ├── rate_limiter.rs      # NEW: adaptive rate limiting
│   ├── extractor.rs         # NEW: __INITIAL_STATE__ extraction
│   ├── commands/
│   │   ├── mod.rs
│   │   ├── search.rs        # NEW
│   │   ├── feed.rs          # NEW
│   │   ├── explore.rs       # NEW
│   │   ├── profile.rs       # NEW
│   │   ├── like.rs          # NEW
│   │   ├── favorite.rs      # NEW
│   │   ├── comment.rs       # NEW
│   │   └── publish.rs       # NEW
│   └── bin/
│       └── xiaohongshu-cli/
│           └── main.rs      # (existing, add subcommands)
├── tests/
│   └── integration/
└── Dockerfile
```

---

## 7. New Dependencies Needed

```toml
[dependencies]
# Existing
chromiumoxide = "0.9.1"
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# New - Phase 1
rand = "0.9"                    # Randomized delays/jitter

# New - Phase 2
chrono = "0.4"                  # Timestamp handling
csv = "1.3"                     # CSV output format
comfy-table = "7"               # Table output format

# New - Phase 3
aes-gcm = "0.10"               # Cookie encryption
argon2 = "0.5"                  # Key derivation for encryption
dialoguer = "0.11"              # Interactive prompts
rpassword = "7"                 # Password input (no echo)
```

---

## 8. Key Risks & Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| XHS updates anti-bot detection | High | Modular design: behavioral params in config file, easy to adjust |
| chromiumoxide stealth mode gaps | High | Consider supplementing with custom JS injection patches |
| Account bans during testing | High | Use test accounts only, start with conservative delays |
| chromiumoxide maintenance status | Medium | Monitor repo; have fallback plan to switch to CDP directly |
| Chrome version compatibility | Medium | Pin Chrome version in Docker; test against multiple versions |
