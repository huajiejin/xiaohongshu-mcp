use std::path::PathBuf;
use std::time::Duration;

const TEST_PROFILE: &str = "xhs-cli-integration-test";

fn xhs_bin() -> String {
    let bin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("xhs");
    assert!(
        bin.exists(),
        "Binary not found at {}. Run `cargo build --bin xhs` first.",
        bin.display()
    );
    bin.to_string_lossy().to_string()
}

fn default_cookie_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xiaohongshu")
        .join("profiles")
        .join("default")
        .join("cookies.json")
}

fn test_profile_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("xiaohongshu")
        .join("profiles")
        .join(TEST_PROFILE)
}

async fn run_xhs(args: &[&str]) -> String {
    let output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new(xhs_bin()).args(args).output(),
    )
    .await
    .expect("Command timed out after 90s")
    .expect("Failed to execute xhs binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        panic!(
            "Command `xhs {}` failed (exit {})\nstdout: {stdout}\nstderr: {stderr}",
            args.join(" "),
            output.status.code().unwrap_or(-1),
        );
    }

    stdout
}

fn copy_cookies_to_test_profile() {
    let src = default_cookie_path();
    assert!(
        src.exists(),
        "No cookies found at {}. Run `cargo run --bin xhs -- auth login` first.",
        src.display()
    );

    let dest_dir = test_profile_dir();
    std::fs::create_dir_all(&dest_dir).unwrap();
    std::fs::copy(&src, dest_dir.join("cookies.json")).unwrap();
}

fn cleanup_test_profile() {
    let dir = test_profile_dir();
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn json_from(output: &str) -> serde_json::Value {
    let line = output
        .lines()
        .find(|l| l.starts_with('{'))
        .unwrap_or(output);
    serde_json::from_str(line)
        .unwrap_or_else(|e| panic!("Failed to parse JSON: {e}\noutput: {output}"))
}

fn base_args() -> Vec<&'static str> {
    vec!["--format", "json", "--profile", TEST_PROFILE]
}

#[tokio::test]
#[ignore]
async fn xhs_cli_integration() {
    copy_cookies_to_test_profile();
    let _guard = scopeguard::guard((), |_| cleanup_test_profile());

    // 1. Auth guard: verify default profile is logged in
    let out = run_xhs(&["auth", "status", "--format", "json"]).await;
    let status: serde_json::Value =
        serde_json::from_str(out.lines().find(|l| l.starts_with('{')).unwrap_or(&out))
            .unwrap_or_else(|e| panic!("Failed to parse auth status JSON: {e}\noutput: {out}"));
    assert!(
        status["logged_in"].as_bool().unwrap_or(false),
        "Not logged in on default profile. Run `cargo run --bin xhs -- auth login` first."
    );

    // 2. Verify test profile cookies work
    let mut args = base_args();
    let mut cmd = vec!["auth", "status"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let status = json_from(&out);
    assert!(
        status["logged_in"].as_bool().unwrap_or(false),
        "Test profile not logged in after cookie copy"
    );

    // 3. Explore
    let mut args = base_args();
    let mut cmd = vec!["explore", "--max-notes", "3"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let explore = json_from(&out);
    assert_eq!(
        explore["total"].as_u64().unwrap_or(0),
        3,
        "explore: expected 3 notes. found: {}",
        explore
    );
    let notes = explore["notes"]
        .as_array()
        .expect("explore: missing notes array");
    assert_eq!(notes.len(), 3, "explore: notes array length != 3");
    for (i, note) in notes.iter().enumerate() {
        assert!(
            !note["note_url"].as_str().unwrap_or("").is_empty(),
            "explore: note {i} has empty note_url"
        );
        assert!(
            !note["creator_profile_url"]
                .as_str()
                .unwrap_or("")
                .is_empty(),
            "explore: note {i} has empty creator_profile_url"
        );
    }
    let explore_note_url = notes[0]["note_url"]
        .as_str()
        .expect("explore: note 0 missing note_url")
        .to_string();
    let creator_url = notes[0]["creator_profile_url"]
        .as_str()
        .expect("explore: note 0 missing creator_profile_url")
        .to_string();

    // 4. Search
    let mut args = base_args();
    let mut cmd = vec![
        "search",
        "英语",
        "--sort-by",
        "most_commented",
        "--max-notes",
        "3",
    ];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let search = json_from(&out);
    let search_total = search["total"].as_u64().unwrap_or(0);
    assert!(
        search_total == 3,
        "search: expected exact 3 notes, got {search_total}"
    );
    let search_notes = search["notes"]
        .as_array()
        .expect("search: missing notes array");
    for (i, note) in search_notes.iter().enumerate() {
        assert!(
            !note["note_url"].as_str().unwrap_or("").is_empty(),
            "search: note {i} has empty note_url"
        );
    }
    let search_note_url = search_notes[0]["note_url"]
        .as_str()
        .unwrap_or(&explore_note_url)
        .to_string();

    // 5. Note detail
    let note_url = if explore_note_url.is_empty() {
        search_note_url
    } else {
        explore_note_url
    };
    assert!(
        !note_url.is_empty(),
        "note: no note_url available from explore or search"
    );
    let mut args = base_args();
    let mut cmd = vec!["note", &note_url, "--max-comments", "5"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let note_detail = json_from(&out);
    let detail = &note_detail["detail"];
    assert!(
        !detail["note_id"].as_str().unwrap_or("").is_empty(),
        "note: empty note_id. {}",
        detail
    );

    // 6. Like
    let mut args = base_args();
    let mut cmd = vec!["like", &note_url];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let like = json_from(&out);
    assert!(
        like["success"].as_bool().unwrap_or(false),
        "like: success != true"
    );

    // 7. Unlike
    let mut args = base_args();
    let mut cmd = vec!["like", &note_url, "--undo"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let unlike = json_from(&out);
    assert!(
        unlike["success"].as_bool().unwrap_or(false),
        "unlike: success != true"
    );

    // 8. Favorite
    let mut args = base_args();
    let mut cmd = vec!["favorite", &note_url];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let fav = json_from(&out);
    assert!(
        fav["success"].as_bool().unwrap_or(false),
        "favorite: success != true"
    );

    // 9. Unfavorite
    let mut args = base_args();
    let mut cmd = vec!["favorite", &note_url, "--undo"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let unfav = json_from(&out);
    assert!(
        unfav["success"].as_bool().unwrap_or(false),
        "unfavorite: success != true"
    );

    // 10. Creator
    assert!(
        !creator_url.is_empty(),
        "creator: no creator_profile_url from explore"
    );
    let mut args = base_args();
    let mut cmd = vec!["creator", &creator_url, "--max-notes", "3"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let creator = json_from(&out);
    let creator_total = creator["total"].as_u64().unwrap_or(0);
    assert!(
        creator_total >= 1,
        "creator: expected at least 1 note, got {creator_total}"
    );
    let creator_notes = creator["notes"]
        .as_array()
        .expect("creator: missing notes array");
    for (i, note) in creator_notes.iter().enumerate() {
        assert!(
            !note["title"].as_str().unwrap_or("").is_empty(),
            "creator: note {i} has empty title"
        );
    }

    // 11. Logout (test profile only)
    let mut args = base_args();
    let mut cmd = vec!["auth", "logout"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let logout = json_from(&out);
    assert!(
        logout["logged_out"].as_bool().unwrap_or(false),
        "logout: logged_out != true"
    );

    // 12. Verify logged out
    let mut args = base_args();
    let mut cmd = vec!["auth", "status"];
    cmd.append(&mut args);
    let out = run_xhs(&cmd).await;
    let status = json_from(&out);
    assert!(
        !status["logged_in"].as_bool().unwrap_or(true),
        "status after logout: still logged in"
    );
}
