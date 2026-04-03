use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::extract::note::{
    CreatorCollectionResult, CreatorInfo, ExtractionRoot, Note, NoteCard, extract_initial_state,
    extract_note_cards_from_initial_state, parse_creator_link, parse_user_info,
};
use crate::t;
use anyhow::{Result, anyhow};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::debug;

pub struct CreatorOptions {
    pub url: String,
    pub max_notes: Option<usize>,
    pub scroll_speed: ScrollSpeed,
    pub duration: Option<u64>,
}

pub async fn run(
    opts: &CreatorOptions,
    browser_opts: &BrowserOptions,
) -> Result<CreatorCollectionResult> {
    let (user_id, _) =
        parse_creator_link(&opts.url).ok_or_else(|| anyhow!("{}", t!("creator.invalid_url")))?;

    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, &opts.url).await?;

    wait_initial_state(&page).await?;

    let state = extract_initial_state(&page).await?;

    let creator = parse_user_info(&state, user_id.clone()).unwrap_or(CreatorInfo {
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
    let mut cards: Vec<NoteCard> = Vec::new();
    let start = Instant::now();

    collect_notes(&page, &mut cards, &mut seen_keys, opts, &mut human, &start).await;

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

    Ok(CreatorCollectionResult {
        creator,
        total,
        notes,
        duration_secs,
        collected_at: collected_at.to_rfc3339(),
    })
}

async fn collect_notes(
    page: &chromiumoxide::page::Page,
    cards: &mut Vec<NoteCard>,
    seen_keys: &mut HashSet<String>,
    opts: &CreatorOptions,
    human: &mut HumanBehavior,
    start: &Instant,
) {
    loop {
        if should_stop(opts, cards.len(), *start) {
            break;
        }

        let batch =
            match extract_note_cards_from_initial_state(page, ExtractionRoot::UserProfile).await {
                Ok(v) => v,
                Err(_) => {
                    human.random_delay(human.config.human_delay.clone()).await;
                    continue;
                }
            };

        let mut new_in_batch = 0;
        for card in batch {
            let key = card_unique_key(&card);
            if seen_keys.contains(&key) {
                continue;
            }
            seen_keys.insert(key);
            new_in_batch += 1;

            let id = card.id.clone().unwrap_or_default();
            debug!("[{}] {} | {}", cards.len() + 1, card.title, id);
            cards.push(card);

            if should_stop(opts, cards.len(), *start) {
                break;
            }
        }

        if should_stop(opts, cards.len(), *start) {
            break;
        }

        if new_in_batch == 0 {
            debug!("no new notes in this scroll, stopping");
            break;
        }

        if let Err(e) = human.scroll_page(page, opts.scroll_speed).await {
            debug!("scroll failed: {e}, stopping");
            break;
        }
        human.random_delay(human.config.human_delay.clone()).await;
    }
}

async fn wait_initial_state(page: &chromiumoxide::page::Page) -> Result<()> {
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

fn should_stop(opts: &CreatorOptions, matched: usize, start: Instant) -> bool {
    if let Some(max) = opts.max_notes
        && matched >= max
    {
        debug!("reached max notes limit ({max})");
        return true;
    }
    if let Some(dur) = opts.duration
        && start.elapsed().as_secs() >= dur
    {
        debug!("reached duration limit ({dur}s)");
        return true;
    }
    false
}

fn card_unique_key(card: &NoteCard) -> String {
    if let Some(id) = &card.id {
        return format!("id:{id}");
    }
    format!("title:{}", card.title)
}
