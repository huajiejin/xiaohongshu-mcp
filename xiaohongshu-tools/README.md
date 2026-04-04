# xiaohongshu-tools

CLI for browsing Xiaohongshu (小红书) with simulated human behavior.

1. Opens a browser window — scan the QR code. Cookies are saved for subsequent commands.
	`cargo run --bin xhs -- auth login`
2. Browse the discover feed.
   `cargo run --bin xhs -- explore --keywords "雅思,英语" --max-notes 3 --interact --duration 5`
3. Search notes by keyword.
   `cargo run --bin xhs -- search "雅思" --sort-by latest --max-notes 3 --interact --duration 5`
4. Collect all notes from a creator's profile.
   `cargo run --bin xhs -- creator <creator_profile_url> --max-notes 3 --duration 5`
5. Get a note's detail with comments.
   `cargo run --bin xhs -- note <note_url> --max-comments 3 --duration 5`

## Common options

| Flag | Description |
|------|-------------|
| `--headless` | Run browser without a visible window |
| `--proxy URL` | Route traffic through a proxy |
| `--format json` | Output as JSON (default: text) |
| `--lang en` | Language: en, zh-CN |
| `--max-notes N` | Stop after N notes (explore/search/creator) |
| `--max-comments N` | Max top-level comments to load (note) |
| `--max-replies N` | Max sub-replies to expand per comment (note) |
| `--duration N` | Auto-exit after N seconds |
| `--interact` | Click into notes and scroll comments |
| `--scroll-speed` | slow, normal (default), fast |

Press Ctrl+C to interrupt any running command.
