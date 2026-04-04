use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::commands::support::{StagnationAction, StagnationTracker, click_show_more_buttons};
use crate::extract::note::{check_note_page_accessible, parse_link_parts};
use crate::shared::utils::ApiResponseWatcher;
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use serde::Serialize;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::debug;

const COMMENT_API_ENDPOINT: &str = "/api/sns/web/v1/comment/post";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentAction {
    Comment,
    Reply,
}

pub struct CommentOptions {
    pub url: String,
    pub text: String,
    pub comment_id: Option<String>,
    pub scroll_speed: ScrollSpeed,
}

#[derive(Serialize)]
pub struct CommentResult {
    pub action: String,
    pub note_id: String,
    pub text: String,
    pub target_comment_id: Option<String>,
    pub posted_comment_id: Option<String>,
    pub success: bool,
}

impl fmt::Display for CommentResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.target_comment_id {
            Some(target_comment_id) => writeln!(
                f,
                "{}",
                t!(
                    "comment.reply_done",
                    note_id = &self.note_id,
                    target_comment_id = &target_comment_id,
                    posted_comment_id = &self.posted_comment_id.clone().unwrap_or_default(),
                )
            )?,
            None => writeln!(
                f,
                "{}",
                t!(
                    "comment.comment_done",
                    note_id = &self.note_id,
                    posted_comment_id = &self.posted_comment_id.clone().unwrap_or_default(),
                )
            )?,
        }
        Ok(())
    }
}

pub async fn run(
    opts: &CommentOptions,
    browser_opts: &BrowserOptions,
    action: CommentAction,
) -> Result<CommentResult> {
    let link_parts = parse_link_parts(&opts.url);
    let note_id = link_parts
        .note_id
        .ok_or_else(|| anyhow!("{}", t!("note.invalid_url")))?;

    let action_label = match action {
        CommentAction::Comment => "comment",
        CommentAction::Reply => "reply",
    };
    debug!("{action_label}: note_id={note_id}");

    let browser = browser::create_browser(browser_opts).await?;
    let page =
        browser::create_page_with_cookies(&browser, &opts.url, &browser_opts.profile).await?;

    wait_note_page(&page).await?;
    check_note_page_accessible(&page).await?;

    let watcher = ApiResponseWatcher::start(&page, COMMENT_API_ENDPOINT).await?;
    let mut human = HumanBehavior::new();

    match action {
        CommentAction::Comment => post_comment(&page, &opts.text, &mut human).await?,
        CommentAction::Reply => {
            let comment_id = opts
                .comment_id
                .as_deref()
                .ok_or_else(|| anyhow!("{}", t!("comment.missing_comment_id")))?;
            post_reply(&page, comment_id, &opts.text, &mut human).await?
        }
    }

    debug!("{action_label}: submitted, waiting for API response");

    let posted_comment_id = match watcher.wait_for_body(&page, Duration::from_secs(10)).await {
        Ok(body) => {
            let id = body["data"]["comment"]["id"]
                .as_str()
                .map(|s| s.to_string());
            debug!("{action_label}: API response received, comment_id={id:?}");
            id
        }
        Err(e) => {
            debug!("{action_label}: API watch failed ({e}), falling back to __INITIAL_STATE__");
            find_comment_id_by_text(&page, &note_id, &opts.text).await
        }
    };

    Ok(CommentResult {
        action: action_label.to_string(),
        note_id,
        text: opts.text.clone(),
        target_comment_id: opts.comment_id.clone(),
        posted_comment_id,
        success: true,
    })
}

async fn find_comment_id_by_text(page: &Page, note_id: &str, text: &str) -> Option<String> {
    use crate::extract::note::{extract_note_detail_map, parse_note_detail_raw};
    tokio::time::sleep(Duration::from_secs(2)).await;
    let detail_map = extract_note_detail_map(page).await.ok()?;
    let raw = parse_note_detail_raw(&detail_map, note_id)?;
    for comment in &raw.comments {
        if comment.content.as_deref() == Some(text) {
            return comment.id.clone();
        }
        for sub in &comment.sub_comments {
            if sub.content.as_deref() == Some(text) {
                return sub.id.clone();
            }
        }
    }
    None
}

async fn post_comment(page: &Page, text: &str, human: &mut HumanBehavior) -> Result<()> {
    let trigger = page
        .find_element("div.input-box div.content-edit span")
        .await
        .map_err(|e| anyhow!("{}", t!("comment.input_not_found", error = e.to_string())))?;
    human.human_click(page, &trigger).await?;

    let input = page
        .find_element("div.input-box div.content-edit p.content-input")
        .await
        .map_err(|e| anyhow!("{}", t!("comment.input_not_found", error = e.to_string())))?;
    human.human_type_text(page, &input, text).await?;

    click_submit(page, human).await
}

async fn post_reply(
    page: &Page,
    comment_id: &str,
    text: &str,
    human: &mut HumanBehavior,
) -> Result<()> {
    scroll_to_comments_area(page).await;
    human.random_delay(human.config.human_delay.clone()).await;

    let comment_el = find_comment_element(page, comment_id, human).await?;

    let reply_btn = comment_el
        .find_element(".right .interactions .reply")
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!("comment.reply_btn_not_found", error = e.to_string())
            )
        })?;
    human.human_click(page, &reply_btn).await?;

    let input = page
        .find_element("div.input-box div.content-edit p.content-input")
        .await
        .map_err(|e| anyhow!("{}", t!("comment.input_not_found", error = e.to_string())))?;
    human.human_type_text(page, &input, text).await?;

    click_submit(page, human).await
}

async fn click_submit(page: &Page, human: &mut HumanBehavior) -> Result<()> {
    let submit = page
        .find_element("div.bottom button.submit")
        .await
        .map_err(|e| anyhow!("{}", t!("comment.submit_not_found", error = e.to_string())))?;
    human.human_click(page, &submit).await?;
    Ok(())
}

async fn find_comment_element(
    page: &Page,
    comment_id: &str,
    human: &mut HumanBehavior,
) -> Result<chromiumoxide::element::Element> {
    let direct_selector = format!("#comment-{comment_id}");
    if let Ok(el) = page.find_element(&direct_selector).await {
        debug!("found comment by id selector: {direct_selector}");
        return Ok(el);
    }

    debug!("comment {comment_id} not visible, scrolling to find it");

    click_show_more_buttons(page, usize::MAX, human).await;

    if let Ok(el) = page.find_element(&direct_selector).await {
        debug!("found comment {comment_id} after initial expand");
        return Ok(el);
    }

    let max_attempts = 100;
    let mut prev_count = 0usize;
    let mut stagnation = StagnationTracker::for_comments();

    for attempt in 0..max_attempts {
        if let Ok(el) = page.find_element(&direct_selector).await {
            debug!("found comment {comment_id} after {attempt} scrolls");
            return Ok(el);
        }

        if attempt % 3 == 0 {
            click_show_more_buttons(page, usize::MAX, human).await;
            if let Ok(el) = page.find_element(&direct_selector).await {
                debug!("found comment {comment_id} after expand at attempt {attempt}");
                return Ok(el);
            }
        }

        let count = count_dom_comments(page).await;
        let made_progress = count != prev_count;
        prev_count = count;

        match stagnation.record(made_progress) {
            StagnationAction::Sprint => {
                debug!("stagnation sprint while searching comment");
                for _ in 0..10 {
                    scroll_note_container(page, 800).await;
                    let _ = human.smart_scroll(page, 600).await;
                    human.random_delay(human.config.scroll_wait.clone()).await;
                }
                stagnation.reset();
                continue;
            }
            StagnationAction::Escalate => {
                for _ in 0..3 {
                    scroll_note_container(page, 800).await;
                    let _ = human.smart_scroll(page, 800).await;
                    human.random_delay(human.config.scroll_wait.clone()).await;
                }
                continue;
            }
            StagnationAction::GiveUp => break,
            StagnationAction::Continue => {}
        }

        scroll_note_container(page, 600).await;
        let _ = human.smart_scroll(page, 600).await;
        human.random_delay(human.config.human_delay.clone()).await;
    }

    Err(anyhow!(
        "{}",
        t!("comment.comment_not_found", comment_id = comment_id)
    ))
}

async fn scroll_note_container(page: &Page, delta: i64) {
    let js = format!(
        r#"(() => {{
            const el = document.querySelector('.note-scroller')
                || document.querySelector('.interaction-container')
                || document.documentElement;
            el.scrollBy({{top: {delta}, behavior: 'smooth'}});
        }})()"#
    );
    let _ = page.evaluate_expression(&js).await;
}

async fn scroll_to_comments_area(page: &Page) {
    let js = r#"(() => {
        const el = document.querySelector('.comments-container')
            || document.querySelector('.interaction-container');
        if (el) {
            el.scrollIntoView({behavior:'smooth', block:'start'});
            const scroller = document.querySelector('.note-scroller') || el;
            scroller.dispatchEvent(new WheelEvent('wheel',{
                deltaY:300,deltaMode:0,bubbles:true,cancelable:true,view:window
            }));
            return true;
        }
        return false;
    })()"#;
    let _ = page.evaluate_expression(js).await;
}

async fn count_dom_comments(page: &Page) -> usize {
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

async fn wait_note_page(page: &Page) -> Result<()> {
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
