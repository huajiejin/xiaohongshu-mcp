use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::commands::support::{
    InteractTracker, StagnationAction, StagnationTracker, StopCondition, interact_with_cards,
    process_card_batch,
};
use crate::extract::note::{
    CollectionResult, ExtractionRoot, Note, NoteCard, extract_note_cards_with_fallback,
};
use crate::selectors::Selector;
use crate::t;
use chromiumoxide::page::Page;
use std::collections::HashSet;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

const EXPLORE_URL: &str = "https://www.xiaohongshu.com/explore";

pub struct ExploreOptions {
    pub keywords: Vec<String>,
    pub exclude: Vec<String>,
    pub max_notes: usize,
    pub scroll_speed: ScrollSpeed,
    pub interact: bool,
    pub duration: u64,
}

pub async fn run(
    opts: &ExploreOptions,
    browser_opts: &BrowserOptions,
    token: &CancellationToken,
) -> Result<CollectionResult, anyhow::Error> {
    let mut browser = browser::create_browser(browser_opts).await?;
    let page =
        browser::create_page_with_cookies(&browser, EXPLORE_URL, &browser_opts.profile).await?;

    check_login(&page).await;

    let keyword_desc = if opts.keywords.is_empty() {
        "all notes".to_string()
    } else {
        opts.keywords.join(",")
    };
    debug!("exploring notes for [{}] (Ctrl+C to stop)", keyword_desc);

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

        let batch = match extract_note_cards_with_fallback(&page, ExtractionRoot::Explore).await {
            Ok(v) => v,
            Err(_) => {
                human.random_delay(human.config.human_delay.clone()).await;
                continue;
            }
        };

        let new_cards = process_card_batch(
            batch,
            &mut seen_keys,
            &opts.keywords,
            &opts.exclude,
            &mut cards,
        );

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

        human.scroll_page(&page, opts.scroll_speed).await?;
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
    debug!("done: explored {} notes in {}s", total, duration_secs);

    browser.close().await?;
    Ok(CollectionResult {
        total,
        notes,
        duration_secs,
        collected_at: collected_at.to_rfc3339(),
    })
}

async fn check_login(page: &Page) {
    let logged_in = page
        .find_element(Selector::LoginIndicator.css())
        .await
        .is_ok();
    if !logged_in {
        warn!("{}", t!("explore.not_logged_in_warn"));
    }
}
