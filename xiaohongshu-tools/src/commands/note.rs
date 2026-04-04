use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::commands::support::{
    StagnationAction, StagnationTracker, StopCondition, click_show_more_buttons,
};
use crate::extract::note::{
    NoteDetail, NoteResult, check_note_page_accessible, extract_note_detail_map, parse_link_parts,
    parse_note_detail_raw,
};
use crate::t;
use anyhow::{Result, anyhow};
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub struct NoteOptions {
    pub url: String,
    pub max_comments: usize,
    pub max_replies: usize,
    pub scroll_speed: ScrollSpeed,
    pub duration: u64,
}

pub async fn run(
    opts: &NoteOptions,
    browser_opts: &BrowserOptions,
    token: &CancellationToken,
) -> Result<NoteResult> {
    let start = Instant::now();

    let link_parts = parse_link_parts(&opts.url);
    let note_id = link_parts
        .note_id
        .ok_or_else(|| anyhow!("{}", t!("note.invalid_url")))?;

    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, &opts.url).await?;

    wait_note_page(&page).await?;
    check_note_page_accessible(&page).await?;

    let detail_map = extract_note_detail_map(&page).await?;
    let mut raw = parse_note_detail_raw(&detail_map, &note_id)
        .ok_or_else(|| anyhow!("{}", t!("note.not_found")))?;

    debug!("extracted note detail: note_id={note_id}");

    let initial_comment_count = raw.comments.len();
    let needs_scroll = opts.max_comments > 0 && initial_comment_count < opts.max_comments;

    if needs_scroll {
        debug!(
            "loading comments: initial={initial_comment_count}, target={}",
            opts.max_comments
        );
        let mut human = HumanBehavior::new();
        load_comments(&page, opts, &mut human, &start, token).await;

        if let Ok(new_map) = extract_note_detail_map(&page).await
            && let Some(new_raw) = parse_note_detail_raw(&new_map, &note_id)
        {
            raw.comments = new_raw.comments;
            raw.comments_cursor = new_raw.comments_cursor;
            raw.comments_has_more = new_raw.comments_has_more;
        }
    }

    let mut detail = NoteDetail::from_raw(&raw);
    detail.comments.truncate(opts.max_comments);
    for comment in &mut detail.comments {
        comment.sub_comments.truncate(opts.max_replies);
    }
    detail.comments_loaded = detail.comments.len();
    detail.note_url = Some(opts.url.clone());

    let duration_secs = start.elapsed().as_secs_f64();
    let collected_at = chrono::Local::now().to_rfc3339();

    debug!(
        "done: note detail collected {} comments in {:.1}s",
        detail.comments_loaded, duration_secs
    );

    Ok(NoteResult {
        detail,
        duration_secs,
        collected_at,
    })
}

async fn load_comments(
    page: &chromiumoxide::page::Page,
    opts: &NoteOptions,
    human: &mut HumanBehavior,
    start: &Instant,
    token: &CancellationToken,
) {
    scroll_to_comments_area(page).await;
    human.random_delay(human.config.human_delay.clone()).await;

    if check_no_comments(page).await {
        debug!("no comments on this note");
        return;
    }

    let max_attempts = (opts.max_comments * 3).max(30);
    let mut prev_count = count_dom_comments(page).await;
    let mut stagnation = StagnationTracker::for_comments();
    let stop = StopCondition::new(opts.max_comments, opts.duration);

    for attempt in 0..max_attempts {
        if token.is_cancelled() {
            debug!("cancelled, stopping");
            break;
        }

        if stop.check(count_dom_comments(page).await, *start) {
            break;
        }

        if check_end_container(page).await {
            debug!("reached end of comments");
            break;
        }

        if attempt % 3 == 0 {
            click_show_more_buttons(page, opts.max_replies, human).await;
        }

        let count = count_dom_comments(page).await;
        if count >= opts.max_comments {
            debug!("reached target comments ({count})");
            break;
        }

        let made_progress = count != prev_count;
        prev_count = count;

        match stagnation.record(made_progress) {
            StagnationAction::Sprint => {
                debug!("stagnation detected, big sprint");
                for _ in 0..10 {
                    let _ = human.scroll_page(page, opts.scroll_speed).await;
                }
                human.random_delay(human.config.human_delay.clone()).await;
                stagnation.reset();
                continue;
            }
            StagnationAction::Escalate => {
                scroll_comments(page, human, opts.scroll_speed, true).await;
                human.random_delay(human.config.human_delay.clone()).await;
                continue;
            }
            _ => {}
        }

        scroll_comments(page, human, opts.scroll_speed, false).await;
        human.random_delay(human.config.human_delay.clone()).await;
    }
}

async fn wait_note_page(page: &chromiumoxide::page::Page) -> Result<()> {
    let js = r#"(() => {
        const s = window.__INITIAL_STATE__;
        if (!s) return false;
        const get_val = (o) => o?.value || o?._value || o?._rawValue;
        const note = get_val(s?.note) || s?.note;
        const dm = get_val(note?.noteDetailMap) || note?.noteDetailMap;
        return dm && typeof dm === 'object';
    })()"#;

    let timeout = Duration::from_secs(15);
    let start = Instant::now();
    loop {
        let ready = page
            .evaluate_expression(js)
            .await
            .ok()
            .and_then(|v| v.into_value::<serde_json::Value>().ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if ready {
            return Ok(());
        }

        if start.elapsed() >= timeout {
            anyhow::bail!("{}", t!("note.wait_timeout"));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn scroll_to_comments_area(page: &chromiumoxide::page::Page) {
    let js = r#"(() => {
        const el = document.querySelector('.comments-container')
            || document.querySelector('.interaction-container');
        if (el) {
            el.scrollIntoView({behavior:'smooth', block:'start'});
            return true;
        }
        return false;
    })()"#;
    let _ = page.evaluate_expression(js).await;
}

async fn check_no_comments(page: &chromiumoxide::page::Page) -> bool {
    let js = r#"(() => {
        const el = document.querySelector('.no-comments-text');
        if (!el) return false;
        return el.innerText.includes('荒地');
    })()"#;
    page.evaluate_expression(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<serde_json::Value>().ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

async fn check_end_container(page: &chromiumoxide::page::Page) -> bool {
    let js = r#"(() => {
        const el = document.querySelector('.end-container');
        if (!el) return false;
        return el.innerText.includes('THE END');
    })()"#;
    page.evaluate_expression(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<serde_json::Value>().ok())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

async fn count_dom_comments(page: &chromiumoxide::page::Page) -> usize {
    let js = r#"(() => {
        return document.querySelectorAll('.parent-comment').length;
    })()"#;
    page.evaluate_expression(js)
        .await
        .ok()
        .and_then(|v| v.into_value::<serde_json::Value>().ok())
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize
}

async fn scroll_comments(
    page: &chromiumoxide::page::Page,
    human: &mut HumanBehavior,
    speed: ScrollSpeed,
    large: bool,
) {
    if large {
        for _ in 0..3 {
            let _ = human.scroll_page(page, speed).await;
        }
    } else {
        let _ = human.scroll_page(page, speed).await;
    }

    let _ = human.smart_scroll(page, 600).await;
}
