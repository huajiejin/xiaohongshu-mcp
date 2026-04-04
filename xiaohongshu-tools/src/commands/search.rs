use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::commands::support::{
    InteractTracker, StagnationAction, StagnationTracker, StopCondition, interact_with_cards,
    process_card_batch,
};
use crate::extract::note::{
    CollectionResult, ExtractionRoot, Note, NoteCard, extract_note_cards_with_fallback,
};
use crate::shared::utils::ApiResponseWatcher;
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use std::collections::HashSet;
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

const SEARCH_URL_TEMPLATE: &str =
    "https://www.xiaohongshu.com/search_result?keyword={keyword}&source=web_explore_feed";

#[derive(Debug, Clone, Copy)]
pub enum SortBy {
    General,
    Latest,
    MostLiked,
    MostCommented,
    MostCollected,
}

impl FromStr for SortBy {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "general" => Ok(Self::General),
            "latest" => Ok(Self::Latest),
            "most_liked" => Ok(Self::MostLiked),
            "most_commented" => Ok(Self::MostCommented),
            "most_collected" => Ok(Self::MostCollected),
            _ => Err(anyhow!(t!("search.invalid_sort_by", val = s))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NoteType {
    All,
    Video,
    Normal,
}

impl FromStr for NoteType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "all" => Ok(Self::All),
            "video" => Ok(Self::Video),
            "normal" => Ok(Self::Normal),
            _ => Err(anyhow!(t!("search.invalid_note_type", val = s))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PublishTime {
    All,
    OneDay,
    OneWeek,
    HalfYear,
}

impl FromStr for PublishTime {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "all" => Ok(Self::All),
            "one_day" => Ok(Self::OneDay),
            "one_week" => Ok(Self::OneWeek),
            "half_year" => Ok(Self::HalfYear),
            _ => Err(anyhow!(t!("search.invalid_publish_time", val = s))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SearchScope {
    All,
    Viewed,
    NotViewed,
    Followed,
}

impl FromStr for SearchScope {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "all" => Ok(Self::All),
            "viewed" => Ok(Self::Viewed),
            "not_viewed" => Ok(Self::NotViewed),
            "followed" => Ok(Self::Followed),
            _ => Err(anyhow!(t!("search.invalid_search_scope", val = s))),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Location {
    All,
    SameCity,
    Nearby,
}

impl FromStr for Location {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "all" => Ok(Self::All),
            "same_city" => Ok(Self::SameCity),
            "nearby" => Ok(Self::Nearby),
            _ => Err(anyhow!(t!("search.invalid_location", val = s))),
        }
    }
}

struct FilterClick {
    group: usize,
    tag: usize,
}

impl FilterClick {
    fn selector(&self) -> String {
        format!(
            "div.filter-panel div.filters:nth-child({}) div.tags:nth-child({})",
            self.group, self.tag
        )
    }
}

trait ToFilterClick {
    fn to_filter_click(&self) -> FilterClick;
}

impl ToFilterClick for SortBy {
    fn to_filter_click(&self) -> FilterClick {
        FilterClick {
            group: 1,
            tag: match self {
                Self::General => 1,
                Self::Latest => 2,
                Self::MostLiked => 3,
                Self::MostCommented => 4,
                Self::MostCollected => 5,
            },
        }
    }
}

impl ToFilterClick for NoteType {
    fn to_filter_click(&self) -> FilterClick {
        FilterClick {
            group: 2,
            tag: match self {
                Self::All => 1,
                Self::Video => 2,
                Self::Normal => 3,
            },
        }
    }
}

impl ToFilterClick for PublishTime {
    fn to_filter_click(&self) -> FilterClick {
        FilterClick {
            group: 3,
            tag: match self {
                Self::All => 1,
                Self::OneDay => 2,
                Self::OneWeek => 3,
                Self::HalfYear => 4,
            },
        }
    }
}

impl ToFilterClick for SearchScope {
    fn to_filter_click(&self) -> FilterClick {
        FilterClick {
            group: 4,
            tag: match self {
                Self::All => 1,
                Self::Viewed => 2,
                Self::NotViewed => 3,
                Self::Followed => 4,
            },
        }
    }
}

impl ToFilterClick for Location {
    fn to_filter_click(&self) -> FilterClick {
        FilterClick {
            group: 5,
            tag: match self {
                Self::All => 1,
                Self::SameCity => 2,
                Self::Nearby => 3,
            },
        }
    }
}

pub struct SearchOptions {
    pub query: String,
    pub sort_by: Option<SortBy>,
    pub note_type: Option<NoteType>,
    pub publish_time: Option<PublishTime>,
    pub search_scope: Option<SearchScope>,
    pub location: Option<Location>,
    pub max_notes: usize,
    pub scroll_speed: ScrollSpeed,
    pub interact: bool,
    pub duration: u64,
}

pub async fn run(
    opts: &SearchOptions,
    browser_opts: &BrowserOptions,
    token: &CancellationToken,
) -> Result<CollectionResult> {
    let url = SEARCH_URL_TEMPLATE.replace("{keyword}", &urlencoding::encode(&opts.query));

    let mut browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, &url).await?;

    wait_initial_state(&page).await?;

    let has_filters = opts.sort_by.is_some()
        || opts.note_type.is_some()
        || opts.publish_time.is_some()
        || opts.search_scope.is_some()
        || opts.location.is_some();

    let mut human = HumanBehavior::new();

    if has_filters {
        apply_filters(&page, opts, &mut human).await?;
    }

    let mut seen_keys: HashSet<String> = HashSet::new();
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

        let batch = match extract_note_cards_with_fallback(&page, ExtractionRoot::Search).await {
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
    debug!("done: searched {} notes in {}s", total, duration_secs);

    browser.close().await?;
    Ok(CollectionResult {
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
            anyhow::bail!("{}", t!("search.wait_initial_state_timeout"));
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn apply_filters(page: &Page, opts: &SearchOptions, human: &mut HumanBehavior) -> Result<()> {
    let watcher =
        ApiResponseWatcher::start(page, "edith.xiaohongshu.com/api/sns/web/v1/search/notes")
            .await?;

    let filter_btn = page
        .find_element("div.filter")
        .await
        .map_err(|_| anyhow!(t!("search.filter_button_not_found")))?;

    let anchor_point = filter_btn.clickable_point().await?;
    page.move_mouse(anchor_point).await?;
    human.random_delay(human.config.reaction_time.clone()).await;

    let panel_ready = wait_for_filter_panel(page).await;
    if !panel_ready {
        filter_btn
            .click()
            .await
            .map_err(|_| anyhow!(t!("search.filter_click_failed")))?;
        human.random_delay(human.config.reaction_time.clone()).await;

        let retry_ready = wait_for_filter_panel(page).await;
        if !retry_ready {
            anyhow::bail!("{}", t!("search.filter_panel_timeout"));
        }
    }

    let clicks: Vec<FilterClick> = collect_filter_clicks(opts);

    for fc in &clicks {
        let selector = fc.selector();

        let anchor_point = filter_btn.clickable_point().await?;
        page.move_mouse(anchor_point).await?;
        human.random_delay(human.config.reaction_time.clone()).await;

        let Ok(el) = page.find_element(&selector).await else {
            warn!(
                "{}",
                t!("search.filter_option_not_found", selector = &selector)
            );
            continue;
        };

        human
            .popup_click(page, &filter_btn, &el)
            .await
            .map_err(|e| {
                anyhow!(
                    "{}",
                    t!(
                        "search.filter_option_click_failed",
                        selector = &selector,
                        e = e.to_string()
                    )
                )
            })?;
    }

    human.random_delay(human.config.read_time.clone()).await;

    watcher.wait(Duration::from_secs(15)).await?;

    Ok(())
}

fn collect_filter_clicks(opts: &SearchOptions) -> Vec<FilterClick> {
    let mut clicks = Vec::new();
    if let Some(v) = &opts.sort_by {
        clicks.push(v.to_filter_click());
    }
    if let Some(v) = &opts.note_type {
        clicks.push(v.to_filter_click());
    }
    if let Some(v) = &opts.publish_time {
        clicks.push(v.to_filter_click());
    }
    if let Some(v) = &opts.search_scope {
        clicks.push(v.to_filter_click());
    }
    if let Some(v) = &opts.location {
        clicks.push(v.to_filter_click());
    }
    clicks
}

async fn wait_for_filter_panel(page: &Page) -> bool {
    let js = r#"(() => {
        return document.querySelector('div.filter-panel') !== null;
    })()"#;

    let timeout = Duration::from_secs(10);
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
            return true;
        }

        if start.elapsed() >= timeout {
            return false;
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::with_capacity(s.len() * 3);
        for byte in s.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    result.push(byte as char);
                }
                _ => {
                    result.push('%');
                    result.push_str(&format!("{:02X}", byte));
                }
            }
        }
        result
    }
}
