use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::commands::interact;
use crate::commands::support::{
    StagnationAction, StagnationTracker, StopCondition, card_unique_key, process_card_batch,
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
use tracing::{debug, warn};

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
    let page = browser::create_page_with_cookies(&browser, &opts.url).await?;

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
    let mut interacted_keys: HashSet<String> = HashSet::new();
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
            for card in &new_cards {
                let key = card_unique_key(card);
                if interacted_keys.contains(&key) {
                    continue;
                }
                interacted_keys.insert(key);

                if let Err(e) = interact_with_note(&page, card, &mut human).await {
                    warn!("{}", t!("explore.explore_note_failed", e = e.to_string()));
                }
            }
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

async fn interact_with_note(
    page: &Page,
    card: &NoteCard,
    human: &mut HumanBehavior,
) -> Result<(), anyhow::Error> {
    interact::open_note(page, card).await?;
    interact::browse_note(page, human).await
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
