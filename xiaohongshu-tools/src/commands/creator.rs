use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::commands::support::{
    InteractTracker, StagnationAction, StagnationTracker, StopCondition, interact_with_cards,
    process_card_batch,
};
use crate::extract::note::{
    CreatorCollectionResult, CreatorInfo, ExtractionRoot, Note, NoteCard, extract_initial_state,
    extract_note_cards_from_initial_state, parse_creator_link, parse_user_info,
};
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::debug;

pub struct CreatorOptions {
    pub url: String,
    pub max_notes: usize,
    pub scroll_speed: ScrollSpeed,
    pub interact: bool,
    pub duration: u64,
}

pub async fn run(
    opts: &CreatorOptions,
    browser_opts: &BrowserOptions,
    token: &CancellationToken,
) -> Result<CreatorCollectionResult> {
    let (user_id, _) =
        parse_creator_link(&opts.url).ok_or_else(|| anyhow!("{}", t!("creator.invalid_url")))?;

    let mut browser = browser::create_browser(browser_opts).await?;
    let page =
        browser::create_page_with_cookies(&browser, &opts.url, &browser_opts.profile).await?;

    wait_initial_state(&page).await?;

    let state = extract_initial_state(&page).await?;

    let creator = parse_user_info(&state, user_id.clone()).unwrap_or_else(|| CreatorInfo {
        user_id: user_id.clone(),
        nickname: None,
        red_id: None,
        desc: None,
        ip_location: None,
        gender: None,
        avatar: None,
        interactions: Vec::new(),
    });

    let mut human = HumanBehavior::new();
    let mut seen_keys = HashSet::new();
    let mut interact_tracker = InteractTracker::new();
    let mut cards: Vec<NoteCard> = Vec::new();
    let start = Instant::now();
    let stop = StopCondition::new(opts.max_notes, opts.duration);
    let mut stagnation = StagnationTracker::for_feed();

    loop {
        if token.is_cancelled() {
            debug!("cancelled, stopping");
            break;
        }

        if stop.check(cards.len(), start) {
            break;
        }

        let batch =
            match extract_note_cards_from_initial_state(&page, ExtractionRoot::UserProfile).await {
                Ok(v) => v,
                Err(_) => {
                    human.random_delay(human.config.human_delay.clone()).await;
                    continue;
                }
            };

        let new_cards = process_card_batch(batch, &mut seen_keys, &[], &[], &mut cards);

        if opts.interact {
            interact_with_cards(&mut interact_tracker, &page, &new_cards, &mut human).await;
        }

        if stop.check(cards.len(), start) {
            break;
        }

        let made_progress = !new_cards.is_empty();
        match stagnation.record(made_progress) {
            StagnationAction::Sprint => {
                debug!("stagnation detected, sprinting");
                for _ in 0..5 {
                    let _ = human.scroll_page(&page, opts.scroll_speed).await;
                }
                human.random_delay(human.config.human_delay.clone()).await;
                stagnation.confirm_sprint();
                continue;
            }
            StagnationAction::GiveUp => {
                debug!("still no new notes after sprint, stopping");
                break;
            }
            _ => {}
        }

        if let Err(e) = human.scroll_page(&page, opts.scroll_speed).await {
            debug!("scroll failed: {e}, stopping");
            break;
        }
        human.random_delay(human.config.human_delay.clone()).await;
    }

    let duration_secs = start.elapsed().as_secs();
    let collected_at = chrono::Local::now();
    let notes: Vec<Note> = cards
        .iter()
        .take(opts.max_notes)
        .map(|c| Note::from_card(c, None, collected_at))
        .collect();
    let total = notes.len();
    debug!(
        "done: creator fetched {} notes in {}s",
        total, duration_secs
    );

    browser.close().await?;
    Ok(CreatorCollectionResult {
        creator,
        total,
        notes,
        duration_secs,
        collected_at: collected_at.to_rfc3339(),
    })
}

async fn wait_initial_state(page: &Page) -> Result<()> {
    let js = r#"(() => {
        return window.__INITIAL_STATE__ !== undefined;
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
            anyhow::bail!("{}", t!("creator.wait_initial_state_timeout"));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
