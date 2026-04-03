use crate::browser::human::{HumanBehavior, ScrollSpeed};
use crate::browser::{self, BrowserOptions};
use crate::extract::note::extract_initial_state;
use crate::extract::note::{
    ExtractionRoot, NoteCard, extract_note_cards_from_initial_state, parse_creator_link,
};
use crate::t;
use anyhow::{Result, anyhow};
use serde::Serialize;
use std::collections::HashSet;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::debug;

#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub nickname: Option<String>,
    pub red_id: Option<String>,
    pub desc: Option<String>,
    pub ip_location: Option<String>,
    pub gender: Option<String>,
    pub avatar: Option<String>,
}

impl fmt::Display for UserInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nickname = self.nickname.as_deref().unwrap_or("-");
        let red_id = self.red_id.as_deref().unwrap_or("-");
        write!(f, "{nickname} (@{red_id})")?;
        if let Some(desc) = &self.desc
            && !desc.is_empty()
        {
            write!(f, " — {desc}")?;
        }
        if let Some(ip) = &self.ip_location
            && !ip.is_empty()
        {
            write!(f, " [{ip}]")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserInteraction {
    pub kind: String,
    pub name: String,
    pub count: String,
}

#[derive(Serialize)]
pub struct CreatorResult {
    pub user_id: String,
    pub user_info: Option<UserInfo>,
    pub interactions: Vec<UserInteraction>,
    pub total: usize,
    pub notes: Vec<NoteCard>,
    pub duration_secs: u64,
    pub collected_at: String,
}

impl fmt::Display for CreatorResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(info) = &self.user_info {
            writeln!(f, "{info}")?;
        }

        if !self.interactions.is_empty() {
            let parts: Vec<String> = self
                .interactions
                .iter()
                .map(|i| format!("{}: {}", i.name, i.count))
                .collect();
            writeln!(f, "{}", parts.join(" | "))?;
        }

        writeln!(
            f,
            "{}",
            t!(
                "creator.matched_notes",
                user_id = &self.user_id,
                count = self.total,
                time = self.duration_secs,
                collected_at = &self.collected_at,
            )
        )?;
        for (i, note) in self.notes.iter().enumerate() {
            let id = note.id.as_deref().unwrap_or("");
            let note_type = note.note_type.as_deref().unwrap_or("-");
            writeln!(
                f,
                "  {}",
                t!(
                    "creator.note_line",
                    index = i + 1,
                    title = &note.title,
                    note_type = note_type,
                    id = id,
                )
            )?;
        }
        Ok(())
    }
}

pub struct CreatorOptions {
    pub url: String,
    pub max_notes: Option<usize>,
    pub scroll_speed: ScrollSpeed,
    pub duration: Option<u64>,
}

pub async fn run(opts: &CreatorOptions, browser_opts: &BrowserOptions) -> Result<CreatorResult> {
    let (user_id, _) =
        parse_creator_link(&opts.url).ok_or_else(|| anyhow!("{}", t!("creator.invalid_url")))?;

    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, &opts.url).await?;

    wait_initial_state(&page).await?;

    let state = extract_initial_state(&page).await?;

    let user_info = parse_user_info(&state);
    let interactions = parse_interactions(&state);

    let mut human = HumanBehavior::new();
    let mut seen_keys = HashSet::new();
    let mut notes: Vec<NoteCard> = Vec::new();
    let start = Instant::now();

    collect_notes(&page, &mut notes, &mut seen_keys, opts, &mut human, &start).await;

    let duration_secs = start.elapsed().as_secs();
    let total = notes.len();
    debug!(
        "done: creator fetched {} notes in {}s",
        total, duration_secs
    );

    Ok(CreatorResult {
        user_id,
        user_info,
        interactions,
        total,
        notes,
        duration_secs,
        collected_at: chrono::Utc::now().to_rfc3339(),
    })
}

async fn collect_notes(
    page: &chromiumoxide::page::Page,
    notes: &mut Vec<NoteCard>,
    seen_keys: &mut HashSet<String>,
    opts: &CreatorOptions,
    human: &mut HumanBehavior,
    start: &Instant,
) {
    loop {
        if should_stop(opts, notes.len(), *start) {
            break;
        }

        let cards =
            match extract_note_cards_from_initial_state(page, ExtractionRoot::UserProfile).await {
                Ok(v) => v,
                Err(_) => {
                    human.random_delay(human.config.human_delay.clone()).await;
                    continue;
                }
            };

        let mut new_in_batch = 0;
        for card in cards {
            let key = card_unique_key(&card);
            if seen_keys.contains(&key) {
                continue;
            }
            seen_keys.insert(key);
            new_in_batch += 1;

            let id = card.id.clone().unwrap_or_default();
            debug!("[{}] {} | {}", notes.len() + 1, card.title, id);
            notes.push(card);

            if should_stop(opts, notes.len(), *start) {
                break;
            }
        }

        if should_stop(opts, notes.len(), *start) {
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

fn parse_user_info(state: &serde_json::Value) -> Option<UserInfo> {
    let data = state.get("user_data")?;
    let basic = data.get("basicInfo")?;

    let gender_num = basic.get("gender").and_then(|v| v.as_i64());
    let gender = gender_num.map(|g| match g {
        0 => "male".to_string(),
        1 => "female".to_string(),
        _ => "unknown".to_string(),
    });

    Some(UserInfo {
        nickname: basic
            .get("nickname")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        red_id: basic
            .get("redId")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        desc: basic
            .get("desc")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        ip_location: basic
            .get("ipLocation")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        gender,
        avatar: basic
            .get("imageb")
            .and_then(|v| v.as_str())
            .or_else(|| basic.get("images").and_then(|v| v.as_str()))
            .map(ToString::to_string),
    })
}

fn parse_interactions(state: &serde_json::Value) -> Vec<UserInteraction> {
    let data = match state.get("user_data") {
        Some(d) => d,
        None => return Vec::new(),
    };

    let arr = match data.get("interactions").and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|item| {
            let kind = item
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let count = item
                .get("count")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            if kind.is_empty() {
                None
            } else {
                Some(UserInteraction { kind, name, count })
            }
        })
        .collect()
}
