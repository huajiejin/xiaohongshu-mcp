use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::extract::note::{ExtractionRoot, NoteCard, extract_note_cards_with_fallback};
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use rand::Rng;
use serde::Serialize;
use std::collections::HashSet;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

const EXPLORE_URL: &str = "https://www.xiaohongshu.com/explore";
const CLOSE_BTN_SELECTOR: &str = "body > div.note-detail-mask > div.close-circle";

const COMMENT_CONTAINER_SELECTORS: &[&str] = &[
    "#noteContainer div.interaction-container > div.note-scroller",
    "#noteContainer div.interaction-container",
    "#noteContainer",
];

#[derive(Serialize)]
pub struct ExploreResult {
    pub total: usize,
    pub notes: Vec<NoteCard>,
    pub duration_secs: u64,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub collected_at: chrono::DateTime<chrono::Utc>,
}

impl fmt::Display for ExploreResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            t!(
                "explore.matched_notes",
                count = self.total,
                time = self.duration_secs,
                collected_at = self
                    .collected_at
                    .with_timezone(&chrono::Local)
                    .format("%Y-%m-%d %H:%M:%S"),
            )
        )?;
        for (i, note) in self.notes.iter().enumerate() {
            let creator = note.creator_name.as_deref().unwrap_or("-");
            let creator_id = note.creator_id.as_deref().unwrap_or("-");
            let profile_url = note
                .creator_profile_url()
                .unwrap_or_else(|| "-".to_string());
            writeln!(
                f,
                "  {}",
                t!(
                    "explore.note_line",
                    index = i + 1,
                    title = &note.title,
                    creator = creator,
                    creator_id = creator_id,
                    profile_url = profile_url.as_str(),
                )
            )?;
        }
        Ok(())
    }
}

pub struct ExploreOptions {
    pub keywords: Vec<String>,
    pub exclude: Vec<String>,
    pub max_notes: Option<usize>,
    pub scroll_speed: ScrollSpeed,
    pub interact: bool,
    pub duration: Option<u64>,
}

pub async fn run(opts: &ExploreOptions, browser_opts: &BrowserOptions) -> Result<ExploreResult> {
    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, EXPLORE_URL).await?;

    check_login(&page).await;

    let keyword_desc = if opts.keywords.is_empty() {
        "all notes".to_string()
    } else {
        opts.keywords.join(",")
    };
    debug!("exploring notes for [{}] (Ctrl+C to stop)", keyword_desc);

    let mut human = HumanBehavior::new();
    let mut seen_keys = HashSet::new();
    let mut interacted_keys = HashSet::new();
    let mut notes: Vec<NoteCard> = Vec::new();
    let start = Instant::now();

    loop {
        if should_stop(opts, notes.len(), start) {
            break;
        }

        let cards = match extract_note_cards_with_fallback(&page, ExtractionRoot::Explore).await {
            Ok(v) => v,
            Err(_) => {
                human.random_delay(human.config.human_delay.clone()).await;
                continue;
            }
        };

        for card in cards {
            let key = card_unique_key(&card);
            if seen_keys.contains(&key) {
                continue;
            }
            seen_keys.insert(key.clone());

            let title = card.title.clone();

            if matches_keywords(&title, &opts.exclude) {
                continue;
            }

            if !opts.keywords.is_empty() && !matches_keywords(&title, &opts.keywords) {
                continue;
            }

            let id = card.id.clone().unwrap_or_default();
            debug!("[{}] {} | {}", notes.len() + 1, title, id);
            notes.push(card.clone());

            if opts.interact
                && !interacted_keys.contains(&key)
                && let Err(e) = explore_note_by_card(&page, &card, &mut human).await
            {
                interacted_keys.insert(key);
                warn!("{}", t!("explore.explore_note_failed", e = e.to_string()));
            } else if opts.interact {
                interacted_keys.insert(key);
            }

            if should_stop(opts, notes.len(), start) {
                break;
            }
        }

        if should_stop(opts, notes.len(), start) {
            break;
        }

        human.scroll_page(&page, opts.scroll_speed).await?;
        human.random_delay(human.config.human_delay.clone()).await;
    }

    let duration_secs = start.elapsed().as_secs();
    let total = notes.len();
    debug!("done: explored {} notes in {}s", total, duration_secs);

    Ok(ExploreResult {
        total,
        notes,
        duration_secs,
        collected_at: chrono::Utc::now(),
    })
}

async fn explore_note_by_card(
    page: &Page,
    card: &NoteCard,
    human: &mut HumanBehavior,
) -> Result<()> {
    click_note(page, card, ExtractionRoot::Explore).await?;

    human.random_delay(human.config.short_read.clone()).await;

    if let Err(e) = human
        .scroll_container(page, COMMENT_CONTAINER_SELECTORS)
        .await
    {
        warn!(
            "{}",
            t!("explore.scroll_comments_failed", e = e.to_string())
        );
    }

    human.random_delay(human.config.human_delay.clone()).await;

    close_detail(page).await;

    let wait = {
        let mut rng = rand::rng();
        rng.random_range(1000u64..2000)
    };
    tokio::time::sleep(Duration::from_millis(wait)).await;

    Ok(())
}

async fn click_note(page: &Page, card: &NoteCard, root: ExtractionRoot) -> Result<()> {
    if let Some(href) = &card.href(Some(root)) {
        let href_js = serde_json::to_string(href)?;
        let js = format!(
            r#"(() => {{
                const target = {href_js};
                const links = Array.from(document.querySelectorAll('a.cover'));
                const found = links.find((el) => el.getAttribute('href') === target || el.href.endsWith(target));
                if (!found) return false;
                found.click();
                return true;
            }})()"#
        );
        let clicked = page
            .evaluate_expression(&js)
            .await?
            .into_value::<serde_json::Value>()?
            .as_bool()
            .unwrap_or(false);
        if clicked {
            return Ok(());
        }
    }

    if let Some(note_id) = &card.id {
        let selector = format!("a.cover[href*='/explore/{note_id}']");
        if let Ok(el) = page.find_element(&selector).await {
            el.click().await?;
            return Ok(());
        }
    }

    Err(anyhow!(t!(
        "explore.click_note_failed",
        e = "note element not found"
    )))
}

async fn close_detail(page: &Page) {
    if let Ok(btn) = page.find_element(CLOSE_BTN_SELECTOR).await
        && btn.click().await.is_ok()
    {
        return;
    }

    let js = r#"
        const btn = document.querySelector('.note-detail-mask .close-circle')
            || document.querySelector('.close-circle');
        if (btn) { btn.click(); }
    "#;
    let _ = page.evaluate_expression(js).await;

    let esc_js = r#"
        document.dispatchEvent(new KeyboardEvent('keydown', {key:'Escape',keyCode:27,bubbles:true}));
    "#;
    let _ = page.evaluate_expression(esc_js).await;
}

fn should_stop(opts: &ExploreOptions, matched: usize, start: Instant) -> bool {
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

async fn check_login(page: &Page) {
    let logged_in = page
        .find_element(".main-container .user .link-wrapper .channel")
        .await
        .is_ok();
    if !logged_in {
        warn!("{}", t!("explore.not_logged_in_warn"));
    }
}

fn card_unique_key(card: &NoteCard) -> String {
    if let Some(id) = &card.id {
        return format!("id:{id}");
    }
    format!("title:{}", card.title)
}

fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let lower = text.to_lowercase();
    keywords.iter().any(|kw| lower.contains(&kw.to_lowercase()))
}
