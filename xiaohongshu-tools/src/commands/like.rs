use crate::browser::human::HumanBehavior;
use crate::browser::{self, BrowserOptions};
use crate::extract::note::{
    check_note_page_accessible, extract_note_detail_map, parse_link_parts, parse_note_detail_raw,
};
use crate::t;
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::fmt;
use std::time::Duration;
use tracing::debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionKind {
    Like,
    Favorite,
}

impl InteractionKind {
    const fn selector(self) -> &'static str {
        match self {
            Self::Like => ".interact-container .left .like-lottie",
            Self::Favorite => ".interact-container .left .reds-icon.collect-icon",
        }
    }

    fn action_name(self, undo: bool) -> String {
        match (self, undo) {
            (Self::Like, false) => "like".to_string(),
            (Self::Like, true) => "unlike".to_string(),
            (Self::Favorite, false) => "favorite".to_string(),
            (Self::Favorite, true) => "unfavorite".to_string(),
        }
    }
}

pub struct LikeOptions {
    pub url: String,
    pub undo: bool,
}

#[derive(Serialize)]
pub struct LikeResult {
    pub action: String,
    pub note_id: String,
    pub success: bool,
    pub skipped: bool,
    pub previous_state: Option<bool>,
    pub new_state: Option<bool>,
}

impl fmt::Display for LikeResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.skipped {
            writeln!(
                f,
                "{}",
                t!(
                    "like.skipped",
                    action = &self.action,
                    note_id = &self.note_id,
                )
            )?;
        } else {
            writeln!(
                f,
                "{}",
                t!("like.done", action = &self.action, note_id = &self.note_id,)
            )?;
        }
        Ok(())
    }
}

pub async fn run(
    opts: &LikeOptions,
    browser_opts: &BrowserOptions,
    kind: InteractionKind,
) -> Result<LikeResult> {
    let link_parts = parse_link_parts(&opts.url);
    let note_id = link_parts
        .note_id
        .ok_or_else(|| anyhow!("{}", t!("note.invalid_url")))?;

    let action = kind.action_name(opts.undo);

    debug!("{action}: note_id={note_id}");

    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, &opts.url).await?;

    wait_note_page(&page).await?;
    check_note_page_accessible(&page).await?;

    let detail_map = extract_note_detail_map(&page).await?;
    let raw = parse_note_detail_raw(&detail_map, &note_id)
        .ok_or_else(|| anyhow!("{}", t!("note.not_found")))?;

    let current_state = match kind {
        InteractionKind::Like => raw.liked,
        InteractionKind::Favorite => raw.collected,
    };

    let target_state = !opts.undo;
    if current_state == Some(target_state) {
        debug!("{action}: already in target state ({target_state}), skipping");
        return Ok(LikeResult {
            action,
            note_id,
            success: true,
            skipped: true,
            previous_state: current_state,
            new_state: current_state,
        });
    }

    let mut human = HumanBehavior::new();
    click_interaction_button(&page, kind, &mut human).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    let new_state = read_interaction_state(&page, &note_id, kind).await;

    if new_state != Some(target_state) {
        debug!("{action}: state did not change after first click, retrying");
        click_interaction_button(&page, kind, &mut human).await?;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let final_state = read_interaction_state(&page, &note_id, kind).await;
    let success = final_state == Some(target_state);

    debug!("{action}: previous={current_state:?}, new={final_state:?}, success={success}");

    Ok(LikeResult {
        action,
        note_id,
        success,
        skipped: false,
        previous_state: current_state,
        new_state: final_state,
    })
}

async fn click_interaction_button(
    page: &chromiumoxide::page::Page,
    kind: InteractionKind,
    human: &mut HumanBehavior,
) -> Result<()> {
    let element = page
        .find_element(kind.selector())
        .await
        .map_err(|e| anyhow!("{}", t!("like.button_not_found", error = e.to_string())))?;

    human.human_click(page, &element).await?;
    Ok(())
}

async fn read_interaction_state(
    page: &chromiumoxide::page::Page,
    note_id: &str,
    kind: InteractionKind,
) -> Option<bool> {
    let detail_map = extract_note_detail_map(page).await.ok()?;
    let raw = parse_note_detail_raw(&detail_map, note_id)?;
    match kind {
        InteractionKind::Like => raw.liked,
        InteractionKind::Favorite => raw.collected,
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
    let start = std::time::Instant::now();
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
