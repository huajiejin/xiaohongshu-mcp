use crate::shared::parse::{parse_count, parse_publish_time};
use crate::t;
use anyhow::Result;
use chromiumoxide::page::Page;
use serde::Serialize;
use std::fmt;

const XHS_DOMAIN: &str = "https://www.xiaohongshu.com";
const EXPLORE_SECTIONS_SELECTOR: &str = "#exploreFeeds section";
const XSEC_SOURCE_PC_SEARCH: &str = "pc_search";
const XSEC_SOURCE_PC_FEED: &str = "pc_feed";
const XSEC_SOURCE_PC_USER: &str = "pc_user";

pub async fn extract_initial_state(page: &Page) -> Result<serde_json::Value> {
    let js = r#"(() => {
        const s = window.__INITIAL_STATE__;
        if (!s) return null;
		const get_val = (o) => o?.value || o?._value || o?._rawValue;
		const flatten = (arr) => Array.isArray(arr) && arr.some(Array.isArray) ? arr.flat() : arr;
		const feed_feeds = flatten(get_val(s?.feed?.feeds));
		const search_feeds = flatten(get_val(s?.search?.feeds));
		const user_notes = flatten(get_val(s?.user?.notes));
		const user_data = get_val(s?.user?.userPageData);
        return JSON.parse(JSON.stringify({feed_feeds, search_feeds, user_notes, user_data}));
    })()"#;

    let value = page
        .evaluate_expression(js)
        .await?
        .into_value::<serde_json::Value>()?;

    if value.is_null() {
        anyhow::bail!("{}", t!("extractor.initial_state_null"));
    }

    Ok(value)
}

#[derive(Debug, Clone, Copy)]
pub enum ExtractionRoot {
    Explore,
    Search,
    UserProfile,
}

impl ExtractionRoot {
    const fn xsec_source(&self) -> &'static str {
        match self {
            Self::Explore => XSEC_SOURCE_PC_FEED,
            Self::Search => XSEC_SOURCE_PC_SEARCH,
            Self::UserProfile => XSEC_SOURCE_PC_USER,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkParts {
    pub raw: String,
    pub note_id: Option<String>,
    pub xsec_token: Option<String>,
    pub xsec_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct NoteCard {
    pub id: Option<String>,
    pub xsec_token: Option<String>,
    pub xsec_source: Option<String>,
    pub publish_time: Option<String>,
    pub note_type: Option<String>,
    pub title: String,
    pub creator_name: Option<String>,
    pub creator_id: Option<String>,
    pub creator_xsec_token: Option<String>,
    pub liked: Option<bool>,
    pub liked_count: Option<String>,
    pub collected: Option<bool>,
    pub collected_count: Option<String>,
    pub comment_count: Option<String>,
    pub shared_count: Option<String>,
    pub cover_url: Option<String>,
    pub video_duration_secs: Option<u64>,
}

impl NoteCard {
    pub fn note_url(&self, xsec_source: Option<&str>) -> Option<String> {
        let xsec_source = xsec_source
            .unwrap_or_else(|| self.xsec_source.as_deref().unwrap_or(XSEC_SOURCE_PC_SEARCH));
        match (&self.id, &self.xsec_token) {
            (Some(note_id), Some(token)) => Some(format!(
                "{XHS_DOMAIN}/explore/{note_id}?xsec_token={token}&xsec_source={xsec_source}"
            )),
            _ => None,
        }
    }

    pub fn creator_profile_url(&self) -> Option<String> {
        match (&self.creator_id, &self.creator_xsec_token) {
            (Some(user_id), Some(token)) => Some(format!(
                "{XHS_DOMAIN}/user/profile/{user_id}?xsec_token={token}&xsec_source=pc_search"
            )),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Note {
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xsec_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xsec_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_type: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_profile_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liked_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collected_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_duration_secs: Option<u64>,
}

impl Note {
    pub fn from_card(
        card: &NoteCard,
        xsec_source: Option<&str>,
        collected_at: chrono::DateTime<chrono::Local>,
    ) -> Self {
        Self {
            id: card.id.clone(),
            xsec_token: card.xsec_token.clone(),
            xsec_source: card.xsec_source.clone(),
            publish_time: card
                .publish_time
                .as_deref()
                .and_then(|raw| parse_publish_time(raw, collected_at))
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
            note_type: card.note_type.clone(),
            title: card.title.clone(),
            note_url: card.note_url(xsec_source),
            creator_name: card.creator_name.clone(),
            creator_id: card.creator_id.clone(),
            creator_profile_url: card.creator_profile_url(),
            liked: card.liked,
            liked_count: card.liked_count.as_deref().and_then(parse_count),
            collected: card.collected,
            collected_count: card.collected_count.as_deref().and_then(parse_count),
            comment_count: card.comment_count.as_deref().and_then(parse_count),
            shared_count: card.shared_count.as_deref().and_then(parse_count),
            cover_url: card.cover_url.clone(),
            video_duration_secs: card.video_duration_secs,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatorInteraction {
    pub kind: String,
    pub name: String,
    pub count: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreatorInfo {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub red_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub interactions: Vec<CreatorInteraction>,
}

impl fmt::Display for CreatorInfo {
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

#[derive(Serialize)]
pub struct CollectionResult {
    pub total: usize,
    pub notes: Vec<Note>,
    pub duration_secs: u64,
    pub collected_at: String,
}

impl fmt::Display for CollectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}",
            t!(
                "collection.matched_notes",
                count = self.total,
                time = self.duration_secs,
                collected_at = &self.collected_at,
            )
        )?;
        for (i, note) in self.notes.iter().enumerate() {
            let creator = note.creator_name.as_deref().unwrap_or("-");
            let creator_id = note.creator_id.as_deref().unwrap_or("-");
            let profile_url = note.creator_profile_url.as_deref().unwrap_or("-");
            writeln!(
                f,
                "  {}",
                t!(
                    "collection.note_line",
                    index = i + 1,
                    title = &note.title,
                    creator = creator,
                    creator_id = creator_id,
                    profile_url = profile_url,
                )
            )?;
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct CreatorCollectionResult {
    pub creator: CreatorInfo,
    pub total: usize,
    pub notes: Vec<Note>,
    pub duration_secs: u64,
    pub collected_at: String,
}

impl fmt::Display for CreatorCollectionResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.creator)?;

        if !self.creator.interactions.is_empty() {
            let parts: Vec<String> = self
                .creator
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
                user_id = &self.creator.user_id,
                count = self.total,
                time = self.duration_secs,
                collected_at = &self.collected_at,
            )
        )?;
        for (i, note) in self.notes.iter().enumerate() {
            let note_type = note.note_type.as_deref().unwrap_or("-");
            let id = note.id.as_deref().unwrap_or("");
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

pub fn parse_user_info(state: &serde_json::Value, user_id: String) -> Option<CreatorInfo> {
    let data = state.get("user_data")?;
    let basic = data.get("basicInfo")?;

    let gender_num = basic.get("gender").and_then(|v| v.as_i64());
    let gender = gender_num.map(|g| match g {
        0 => "male".to_string(),
        1 => "female".to_string(),
        _ => "unknown".to_string(),
    });

    Some(CreatorInfo {
        user_id,
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
        interactions: parse_interactions(state),
    })
}

pub fn parse_interactions(state: &serde_json::Value) -> Vec<CreatorInteraction> {
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
                Some(CreatorInteraction { kind, name, count })
            }
        })
        .collect()
}

pub fn parse_creator_link(link: &str) -> Option<(String, String)> {
    let path = link
        .split_once("xiaohongshu.com")
        .map(|(_, r)| r)
        .unwrap_or(link);
    let (path_part, query) = path.split_once('?').unwrap_or((path, ""));
    let user_id = path_part
        .trim_end_matches('/')
        .rsplit('/')
        .next()?
        .to_string();
    let mut xsec_token = String::new();
    for part in query.split('&') {
        if let Some((k, v)) = part.split_once('=')
            && k == "xsec_token"
        {
            xsec_token = v.to_string();
        }
    }
    Some((user_id, xsec_token))
}

pub async fn extract_note_cards_with_fallback(
    page: &Page,
    root: ExtractionRoot,
) -> Result<Vec<NoteCard>> {
    let state_cards = extract_note_cards_from_initial_state(page, root)
        .await
        .unwrap_or_default();
    if !state_cards.is_empty() {
        return Ok(state_cards);
    }

    extract_note_cards_from_dom(page).await
}

pub async fn extract_note_cards_from_initial_state(
    page: &Page,
    root: ExtractionRoot,
) -> Result<Vec<NoteCard>> {
    let state = extract_initial_state(page).await?;

    let notes = match root {
        ExtractionRoot::Explore => state.get("feed_feeds"),
        ExtractionRoot::Search => state.get("search_feeds"),
        ExtractionRoot::UserProfile => state.get("user_notes"),
    };

    let Some(notes) = notes else {
        return Ok(Vec::new());
    };

    let Some(notes) = notes.as_array() else {
        return Ok(Vec::new());
    };

    let xsec_source = root.xsec_source();

    let mut cards = Vec::with_capacity(notes.len());
    for item in notes {
        if let Some(mut card) = card_from_state_item(item) {
            card.xsec_source = Some(xsec_source.to_string());
            cards.push(card);
        }
    }

    Ok(cards)
}

pub async fn extract_note_cards_from_dom(page: &Page) -> Result<Vec<NoteCard>> {
    let sections = match page.find_elements(EXPLORE_SECTIONS_SELECTOR).await {
        Ok(v) => v,
        Err(_) => return Ok(Vec::new()),
    };

    let mut cards = Vec::with_capacity(sections.len());
    for section in &sections {
        let href_raw = match section.find_element("a.cover").await {
            Ok(a) => a.attribute("href").await.ok().flatten(),
            Err(_) => None,
        };
        let Some(href_raw) = href_raw else {
            continue;
        };

        let title = match section.find_element("div > div > a > span").await {
            Ok(span) => span.inner_text().await.ok().flatten().unwrap_or_default(),
            Err(_) => String::new(),
        };

        let link_parts = parse_link_parts(&href_raw);

        cards.push(NoteCard {
            id: link_parts.note_id.clone(),
            xsec_token: link_parts.xsec_token.clone(),
            title,
            xsec_source: link_parts.xsec_source,
            ..Default::default()
        });
    }

    Ok(cards)
}

pub fn parse_link_parts(raw_link: &str) -> LinkParts {
    let (path, query) = raw_link
        .split_once('?')
        .map_or_else(|| (raw_link.to_string(), ""), |(p, q)| (p.to_string(), q));

    let normalized_path = if path.starts_with("http://") || path.starts_with("https://") {
        match path.split_once("xiaohongshu.com") {
            Some((_, rest)) if !rest.is_empty() => rest.to_string(),
            _ => path,
        }
    } else {
        path
    };

    let normalized_path = normalized_path.trim_end_matches('/');
    let note_id = normalized_path
        .split('/')
        .next_back()
        .filter(|&s| !s.is_empty())
        .map(ToString::to_string);

    let mut xsec_token: Option<String> = None;
    let mut xsec_source: Option<String> = None;

    for part in query.split('&') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        if k == "xsec_token" && !v.is_empty() {
            xsec_token = Some(v.to_string());
        }
        if k == "xsec_source" && !v.is_empty() {
            xsec_source = Some(v.to_string());
        }
    }

    LinkParts {
        raw: raw_link.to_string(),
        note_id,
        xsec_token,
        xsec_source,
    }
}

fn card_from_state_item(item: &serde_json::Value) -> Option<NoteCard> {
    let id = item
        .get("id")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let xsec_token = item
        .get("xsecToken")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let note_card = item.get("noteCard")?;
    let title = note_card
        .get("displayTitle")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    let note_type = note_card
        .get("type")
        .and_then(|v| v.as_str())
        .map(ToString::to_string);

    let user = note_card.get("user");
    let creator_name = user
        .and_then(|u| {
            u.get("nickName")
                .and_then(|v| v.as_str())
                .or_else(|| u.get("nickname").and_then(|v| v.as_str()))
        })
        .map(ToString::to_string);
    let creator_id = user
        .and_then(|u| u.get("userId").and_then(|v| v.as_str()))
        .map(ToString::to_string);
    let creator_xsec_token = user
        .and_then(|u| u.get("xsecToken").and_then(|v| v.as_str()))
        .map(ToString::to_string);

    let interact = note_card.get("interactInfo");
    let liked = interact
        .and_then(|i| i.get("liked"))
        .and_then(serde_json::Value::as_bool);
    let liked_count = interact
        .and_then(|i| i.get("likedCount"))
        .and_then(value_to_text);
    let collected = interact
        .and_then(|i| i.get("collected"))
        .and_then(serde_json::Value::as_bool);
    let collected_count = interact
        .and_then(|i| i.get("collectedCount"))
        .and_then(value_to_text);
    let comment_count = interact
        .and_then(|i| i.get("commentCount"))
        .and_then(value_to_text);
    let shared_count = interact
        .and_then(|i| i.get("sharedCount"))
        .and_then(value_to_text);

    let cover_url = pick_best_cover_url(note_card.get("cover"));
    let video_duration_secs = note_card
        .get("video")
        .and_then(|v| v.get("capa"))
        .and_then(|v| v.get("duration"))
        .and_then(serde_json::Value::as_u64);
    let publish_time = note_card
        .get("cornerTagInfo")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|item| item.get("type").and_then(|t| t.as_str()) == Some("publish_time"))
        })
        .and_then(|item| item.get("text").and_then(|t| t.as_str()))
        .map(ToString::to_string);

    Some(NoteCard {
        id,
        xsec_token,
        xsec_source: None,
        publish_time,
        note_type,
        title,
        creator_name,
        creator_id,
        creator_xsec_token,
        liked,
        liked_count,
        collected,
        collected_count,
        comment_count,
        shared_count,
        cover_url,
        video_duration_secs,
    })
}

fn pick_best_cover_url(cover: Option<&serde_json::Value>) -> Option<String> {
    let cover = cover?;

    for key in ["urlDefault", "urlPre", "url"] {
        if let Some(v) = cover.get(key).and_then(|v| v.as_str())
            && !v.is_empty()
        {
            return Some(v.to_string());
        }
    }

    let info_list = cover.get("infoList")?.as_array()?;

    for scene in ["WB_DFT", "WB_PRV"] {
        for item in info_list {
            if item.get("imageScene").and_then(|v| v.as_str()) == Some(scene)
                && let Some(url) = item.get("url").and_then(|v| v.as_str())
                && !url.is_empty()
            {
                return Some(url.to_string());
            }
        }
    }

    for item in info_list {
        if let Some(url) = item.get("url").and_then(|v| v.as_str())
            && !url.is_empty()
        {
            return Some(url.to_string());
        }
    }

    None
}

fn value_to_text(value: &serde_json::Value) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(n) = value.as_u64() {
        return Some(n.to_string());
    }
    if let Some(n) = value.as_i64() {
        return Some(n.to_string());
    }
    None
}

#[derive(Debug, Clone, Serialize)]
pub struct NoteImage {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live_photo: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct CommentRaw {
    pub id: Option<String>,
    pub note_id: Option<String>,
    pub content: Option<String>,
    pub like_count: Option<String>,
    pub create_time: Option<String>,
    pub ip_location: Option<String>,
    pub liked: Option<bool>,
    pub user_name: Option<String>,
    pub user_id: Option<String>,
    pub user_avatar: Option<String>,
    pub sub_comment_count: Option<String>,
    pub sub_comments: Vec<CommentRaw>,
    pub show_tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comment {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_id: Option<String>,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub like_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_comment_count: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sub_comments: Vec<Comment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub show_tags: Vec<String>,
}

impl Comment {
    pub fn from_raw(raw: &CommentRaw) -> Self {
        Self {
            id: raw.id.clone().unwrap_or_default(),
            note_id: raw.note_id.clone(),
            content: raw.content.clone().unwrap_or_default(),
            like_count: raw.like_count.as_deref().and_then(parse_count),
            create_time: raw.create_time.as_deref().and_then(parse_timestamp_ms),
            ip_location: raw.ip_location.clone(),
            liked: raw.liked,
            user_name: raw.user_name.clone(),
            user_id: raw.user_id.clone(),
            user_avatar: raw.user_avatar.clone(),
            sub_comment_count: raw.sub_comment_count.as_deref().and_then(parse_count),
            sub_comments: raw.sub_comments.iter().map(Self::from_raw).collect(),
            show_tags: raw.show_tags.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct NoteDetailRaw {
    pub note_id: Option<String>,
    pub xsec_token: Option<String>,
    pub title: Option<String>,
    pub desc: Option<String>,
    pub note_type: Option<String>,
    pub time: Option<String>,
    pub ip_location: Option<String>,
    pub creator_id: Option<String>,
    pub creator_xsec_token: Option<String>,
    pub creator_name: Option<String>,
    pub creator_avatar: Option<String>,
    pub liked: Option<bool>,
    pub liked_count: Option<String>,
    pub collected: Option<bool>,
    pub collected_count: Option<String>,
    pub comment_count: Option<String>,
    pub shared_count: Option<String>,
    pub images: Vec<NoteImage>,
    pub comments: Vec<CommentRaw>,
    pub comments_cursor: Option<String>,
    pub comments_has_more: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NoteDetail {
    pub note_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xsec_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub publish_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_xsec_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liked_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collected_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_count: Option<u64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<NoteImage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<Comment>,
    pub comments_loaded: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments_has_more: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note_url: Option<String>,
}

impl NoteDetail {
    pub fn from_raw(raw: &NoteDetailRaw) -> Self {
        let comments: Vec<Comment> = raw.comments.iter().map(Comment::from_raw).collect();
        let comments_loaded = comments.len();
        Self {
            note_id: raw.note_id.clone().unwrap_or_default(),
            xsec_token: raw.xsec_token.clone(),
            title: raw.title.clone(),
            desc: raw.desc.clone(),
            note_type: raw.note_type.clone(),
            publish_time: raw.time.as_deref().and_then(parse_timestamp_ms),
            ip_location: raw.ip_location.clone(),
            creator_id: raw.creator_id.clone(),
            creator_xsec_token: raw.creator_xsec_token.clone(),
            creator_name: raw.creator_name.clone(),
            creator_avatar: raw.creator_avatar.clone(),
            liked: raw.liked,
            liked_count: raw.liked_count.as_deref().and_then(parse_count),
            collected: raw.collected,
            collected_count: raw.collected_count.as_deref().and_then(parse_count),
            comment_count: raw.comment_count.as_deref().and_then(parse_count),
            shared_count: raw.shared_count.as_deref().and_then(parse_count),
            images: raw.images.clone(),
            video_url: None,
            comments,
            comments_loaded,
            comments_has_more: raw.comments_has_more,
            note_url: None,
        }
    }
}

#[derive(Serialize)]
pub struct NoteResult {
    pub detail: NoteDetail,
    pub duration_secs: f64,
    pub collected_at: String,
}

impl fmt::Display for NoteResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = &self.detail;
        let title = d.title.as_deref().unwrap_or("-");
        let note_type = d.note_type.as_deref().unwrap_or("-");
        let creator = d.creator_name.as_deref().unwrap_or("-");
        let creator_id = d.creator_id.as_deref().unwrap_or("-");

        writeln!(f, "{}", title)?;
        writeln!(
            f,
            "{}",
            t!(
                "note.header",
                note_type = note_type,
                creator = creator,
                creator_id = creator_id,
            )
        )?;

        if let Some(pt) = &d.publish_time {
            let ip = d
                .ip_location
                .as_deref()
                .map(|ip| format!(" [{ip}]"))
                .unwrap_or_default();
            writeln!(f, "{pt}{ip}")?;
        }

        if !d.images.is_empty() {
            writeln!(f, "{}", t!("note.images_count", count = d.images.len()))?;
            for (i, img) in d.images.iter().enumerate() {
                let dims = match (img.width, img.height) {
                    (Some(w), Some(h)) => format!(" ({w}x{h})"),
                    _ => String::new(),
                };
                writeln!(f, "  {}. {url}{dims}", i + 1, url = img.url)?;
            }
        }

        if let Some(vurl) = &d.video_url {
            writeln!(f, "{}", t!("note.video_url", url = vurl))?;
        }

        if let Some(desc) = &d.desc
            && !desc.is_empty()
        {
            writeln!(f)?;
            for line in desc.lines() {
                writeln!(f, "{line}")?;
            }
        }

        writeln!(f)?;
        writeln!(
            f,
            "{}",
            t!(
                "note.engagement",
                liked = d.liked_count.unwrap_or(0),
                collected = d.collected_count.unwrap_or(0),
                comments = d.comment_count.unwrap_or(0),
                shared = d.shared_count.unwrap_or(0),
            )
        )?;

        if !d.comments.is_empty() {
            let more = if d.comments_has_more.unwrap_or(false) {
                t!("note.has_more").to_string()
            } else {
                String::new()
            };
            writeln!(
                f,
                "{}",
                t!(
                    "note.comments_summary",
                    loaded = d.comments_loaded,
                    more = more,
                )
            )?;
            for (i, c) in d.comments.iter().enumerate() {
                let name = c.user_name.as_deref().unwrap_or("-");
                let likes = c.like_count.unwrap_or(0);
                let ip = c
                    .ip_location
                    .as_deref()
                    .map(|ip| format!(" [{ip}]"))
                    .unwrap_or_default();
                let tags = if c.show_tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", c.show_tags.join(","))
                };
                writeln!(
                    f,
                    "  {}",
                    t!(
                        "note.comment_line",
                        index = i + 1,
                        user = name,
                        content = &c.content,
                        likes = likes,
                        ip = ip,
                        tags = tags,
                    )
                )?;
                for sc in &c.sub_comments {
                    let sname = sc.user_name.as_deref().unwrap_or("-");
                    let slikes = sc.like_count.unwrap_or(0);
                    writeln!(
                        f,
                        "    {}",
                        t!(
                            "note.sub_comment_line",
                            user = sname,
                            content = &sc.content,
                            likes = slikes,
                        )
                    )?;
                }
            }
        }

        writeln!(
            f,
            "{}",
            t!(
                "note.footer",
                time = format!("{:.1}", self.duration_secs),
                collected_at = &self.collected_at,
            )
        )?;

        Ok(())
    }
}

pub async fn extract_note_detail_map(page: &Page) -> Result<serde_json::Value> {
    let js = r#"(() => {
        const s = window.__INITIAL_STATE__;
        if (!s) return null;
        const get_val = (o) => o?.value || o?._value || o?._rawValue;
        const note = get_val(s?.note) || s?.note;
        const detailMap = get_val(note?.noteDetailMap) || note?.noteDetailMap;
        if (!detailMap || typeof detailMap !== 'object') return null;
        return JSON.parse(JSON.stringify(detailMap));
    })()"#;

    let result = page.evaluate_expression(js).await?;
    match result.value() {
        Some(v) if !v.is_null() => Ok(v.clone()),
        _ => anyhow::bail!("{}", t!("extractor.initial_state_null")),
    }
}

pub fn parse_note_detail_raw(
    detail_map: &serde_json::Value,
    note_id: &str,
) -> Option<NoteDetailRaw> {
    let entry = detail_map.get(note_id)?;
    let note = entry.get("note")?;
    let comments_data = entry.get("comments");

    let user = note.get("user");
    let interact = note.get("interactInfo");

    Some(NoteDetailRaw {
        note_id: str_field(note, "noteId"),
        xsec_token: str_field(note, "xsecToken"),
        title: str_field(note, "title"),
        desc: str_field(note, "desc"),
        note_type: str_field(note, "type"),
        time: note
            .get("time")
            .and_then(serde_json::Value::as_i64)
            .map(|t| t.to_string()),
        ip_location: str_field(note, "ipLocation"),
        creator_id: user.and_then(|u| str_field(u, "userId")),
        creator_xsec_token: user.and_then(|u| str_field(u, "xsecToken")),
        creator_name: user
            .and_then(|u| str_field(u, "nickname").or_else(|| str_field(u, "nickName"))),
        creator_avatar: user.and_then(|u| str_field(u, "avatar")),
        liked: interact.and_then(|i| i.get("liked").and_then(serde_json::Value::as_bool)),
        liked_count: interact
            .and_then(|i| i.get("likedCount"))
            .and_then(value_to_text),
        collected: interact.and_then(|i| i.get("collected").and_then(serde_json::Value::as_bool)),
        collected_count: interact
            .and_then(|i| i.get("collectedCount"))
            .and_then(value_to_text),
        comment_count: interact
            .and_then(|i| i.get("commentCount"))
            .and_then(value_to_text),
        shared_count: interact
            .and_then(|i| i.get("sharedCount"))
            .and_then(value_to_text),
        images: parse_image_list(note.get("imageList")),
        comments: comments_data
            .and_then(|c| c.get("list"))
            .and_then(|l| l.as_array())
            .map(|arr| arr.iter().filter_map(parse_comment_raw).collect())
            .unwrap_or_default(),
        comments_cursor: comments_data.and_then(|c| str_field(c, "cursor")),
        comments_has_more: comments_data
            .and_then(|c| c.get("hasMore"))
            .and_then(serde_json::Value::as_bool),
    })
}

fn parse_image_list(image_list: Option<&serde_json::Value>) -> Vec<NoteImage> {
    let arr = match image_list.and_then(serde_json::Value::as_array) {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|item| {
            let url = ["urlDefault", "urlPre"].iter().find_map(|key| {
                item.get(*key)
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
            })?;
            Some(NoteImage {
                url: url.to_string(),
                width: item.get("width").and_then(serde_json::Value::as_u64),
                height: item.get("height").and_then(serde_json::Value::as_u64),
                live_photo: item.get("livePhoto").and_then(serde_json::Value::as_bool),
            })
        })
        .collect()
}

fn parse_comment_raw(value: &serde_json::Value) -> Option<CommentRaw> {
    let id = str_field(value, "id");
    let content = str_field(value, "content");

    if id.is_none() && content.is_none() {
        return None;
    }

    let user_info = value.get("userInfo");

    Some(CommentRaw {
        id,
        note_id: str_field(value, "noteId"),
        content,
        like_count: value.get("likeCount").and_then(value_to_text),
        create_time: value
            .get("createTime")
            .and_then(serde_json::Value::as_i64)
            .map(|t| t.to_string()),
        ip_location: str_field(value, "ipLocation"),
        liked: value.get("liked").and_then(serde_json::Value::as_bool),
        user_name: user_info
            .and_then(|u| str_field(u, "nickname").or_else(|| str_field(u, "nickName"))),
        user_id: user_info.and_then(|u| str_field(u, "userId")),
        user_avatar: user_info.and_then(|u| str_field(u, "avatar")),
        sub_comment_count: value.get("subCommentCount").and_then(value_to_text),
        sub_comments: value
            .get("subComments")
            .and_then(serde_json::Value::as_array)
            .map(|arr| arr.iter().filter_map(parse_comment_raw).collect())
            .unwrap_or_default(),
        show_tags: value
            .get("showTags")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn str_field(parent: &serde_json::Value, key: &str) -> Option<String> {
    parent
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

pub async fn extract_video_url_from_dom(page: &Page) -> Result<Option<String>> {
    let js = r#"(() => {
        const video = document.querySelector('video');
        if (!video) return null;
        return video.src || video.querySelector('source')?.src || null;
    })()"#;

    let result = page.evaluate_expression(js).await?;
    match result.value() {
        Some(serde_json::Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        _ => Ok(None),
    }
}

fn parse_timestamp_ms(raw: &str) -> Option<String> {
    let ms: i64 = raw.parse().ok()?;
    let secs = if ms > 1_000_000_000_000 {
        ms / 1000
    } else {
        ms
    };
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.naive_local().format("%Y-%m-%dT%H:%M:%S").to_string())
}

pub async fn check_note_page_accessible(page: &Page) -> Result<()> {
    let js = r#"(() => {
        const body = document.body?.innerText || '';
        const checks = [
            {text: '笔记已删除', reason: 'deleted'},
            {text: '笔记无法查看', reason: 'private'},
            {text: '笔记违规', reason: 'violation'},
            {text: '该笔记已失效', reason: 'expired'},
            {text: '内容不可见', reason: 'invisible'},
            {text: '该内容因违规', reason: 'violation'},
        ];
        for (const c of checks) {
            if (body.includes(c.text)) return c.reason;
        }
        return null;
    })()"#;

    let result = page.evaluate_expression(js).await?;
    if let Some(reason) = result.value().and_then(serde_json::Value::as_str) {
        anyhow::bail!("{}", t!("note.page_blocked", reason = reason));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    #[test]
    fn test_parse_link_parts() {
        let parts =
            parse_link_parts("/explore/69bbf98d000000001a03241b?xsec_token=abc123&xsec_source=");
        assert_eq!(parts.note_id.as_deref(), Some("69bbf98d000000001a03241b"));
        assert_eq!(parts.xsec_token.as_deref(), Some("abc123"));
        assert_eq!(parts.xsec_source.as_deref(), None);
    }

    #[test]
    fn test_parse_link_parts_from_search_result() {
        let parts = parse_link_parts(
            "/search_result/03bbf95d000000001a022f45?xsec_token=iu872&xsec_source=pc_search",
        );
        assert_eq!(parts.note_id.as_deref(), Some("03bbf95d000000001a022f45"));
        assert_eq!(parts.xsec_token.as_deref(), Some("iu872"));
        assert_eq!(parts.xsec_source.as_deref(), Some("pc_search"));
    }

    #[test]
    fn test_pick_best_cover_url_prefers_url_default() {
        let cover = json!({
            "urlDefault": "http://dft",
            "urlPre": "http://pre",
            "infoList": [
                {"imageScene":"WB_PRV","url":"http://prv"},
                {"imageScene":"WB_DFT","url":"http://listdft"}
            ]
        });
        let got = pick_best_cover_url(Some(&cover));
        assert_eq!(got.as_deref(), Some("http://dft"));
    }

    #[test]
    fn test_card_from_state_item_extracts_key_fields() {
        let item = json!({
            "id": "69cc5c0e000000002200d1ee",
            "xsecToken": "token_note",
            "noteCard": {
                "type": "normal",
                "displayTitle": "title_1",
                "user": {
                    "nickName": "author_1",
                    "userId": "user_1",
                    "xsecToken": "token_author"
                },
                "interactInfo": {
                    "liked": false,
                    "likedCount": "7",
                    "commentCount": "3",
                    "sharedCount": "1",
                    "collectedCount": "6"
                },
                "cover": {
                    "urlDefault": "http://cover_1"
                },
                "video": {
                    "capa": {
                        "duration": 346
                    }
                }
            }
        });
        let card = card_from_state_item(&item).expect("card");
        assert_eq!(card.id.as_deref(), Some("69cc5c0e000000002200d1ee"));
        assert_eq!(card.xsec_token.as_deref(), Some("token_note"));
        assert_eq!(card.title, "title_1");
        assert_eq!(card.creator_name.as_deref(), Some("author_1"));
        assert_eq!(card.creator_id.as_deref(), Some("user_1"));
        assert_eq!(card.creator_xsec_token.as_deref(), Some("token_author"));
        assert_eq!(card.liked, Some(false));
        assert_eq!(card.liked_count.as_deref(), Some("7"));
        assert_eq!(card.comment_count.as_deref(), Some("3"));
        assert_eq!(card.shared_count.as_deref(), Some("1"));
        assert_eq!(card.collected_count.as_deref(), Some("6"));
        assert_eq!(card.cover_url.as_deref(), Some("http://cover_1"));
        assert_eq!(card.video_duration_secs, Some(346));
    }

    #[test]
    fn test_creator_profile_link() {
        let card = NoteCard {
            id: Some("note1".to_string()),
            xsec_token: Some("note_tok".to_string()),
            title: "test".to_string(),
            creator_name: Some("alice".to_string()),
            creator_id: Some("user123".to_string()),
            creator_xsec_token: Some("creator_tok".to_string()),
            ..Default::default()
        };
        assert_eq!(
            card.creator_profile_url(),
            Some("https://www.xiaohongshu.com/user/profile/user123?xsec_token=creator_tok&xsec_source=pc_search".to_string())
        );
    }

    #[test]
    fn test_note_url() {
        let card = NoteCard {
            id: Some("note1".to_string()),
            xsec_token: Some("tok".to_string()),
            xsec_source: Some(XSEC_SOURCE_PC_FEED.to_string()),
            title: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(
            card.note_url(None),
            Some(
                "https://www.xiaohongshu.com/explore/note1?xsec_token=tok&xsec_source=pc_feed"
                    .to_string()
            )
        );

        let card = NoteCard {
            id: Some("note1".to_string()),
            xsec_token: Some("tok".to_string()),
            xsec_source: None,
            title: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(
            card.note_url(None),
            Some(
                "https://www.xiaohongshu.com/explore/note1?xsec_token=tok&xsec_source=pc_search"
                    .to_string()
            )
        );

        let card = NoteCard {
            id: Some("note1".to_string()),
            xsec_token: Some("tok".to_string()),
            xsec_source: Some(XSEC_SOURCE_PC_FEED.to_string()),
            title: "test".to_string(),
            ..Default::default()
        };
        assert_eq!(
            card.note_url(Some(XSEC_SOURCE_PC_USER)),
            Some(
                "https://www.xiaohongshu.com/explore/note1?xsec_token=tok&xsec_source=pc_user"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_parse_creator_link() {
        let url = "https://www.xiaohongshu.com/user/profile/63be318800000000270292d1?xsec_token=ABUsqD9zJZDrpBRj0vEgZ9lwORpvS2c3RJj5QNg6MlejI=&xsec_source=pc_feed";
        let (uid, token) = parse_creator_link(url).unwrap();
        assert_eq!(uid, "63be318800000000270292d1");
        assert_eq!(token, "ABUsqD9zJZDrpBRj0vEgZ9lwORpvS2c3RJj5QNg6MlejI=");
    }

    #[test]
    fn test_parse_creator_link_no_query() {
        let (uid, token) =
            parse_creator_link("https://www.xiaohongshu.com/user/profile/abc123").unwrap();
        assert_eq!(uid, "abc123");
        assert_eq!(token, "");
    }

    #[test]
    fn test_note_from_card_normalizes_counts() {
        let card = NoteCard {
            id: Some("note1".to_string()),
            xsec_token: Some("tok".to_string()),
            xsec_source: Some("pc_search".to_string()),
            title: "test note".to_string(),
            creator_name: Some("alice".to_string()),
            creator_id: Some("user1".to_string()),
            creator_xsec_token: Some("ctok".to_string()),
            liked_count: Some("1,430".to_string()),
            collected_count: Some("1.2万".to_string()),
            comment_count: Some("42".to_string()),
            shared_count: Some("3k".to_string()),
            publish_time: Some("2天前".to_string()),
            ..Default::default()
        };

        let ref_time = chrono::Local
            .with_ymd_and_hms(2026, 4, 3, 12, 0, 0)
            .single()
            .unwrap();
        let note = Note::from_card(&card, None, ref_time);

        assert_eq!(note.id, Some("note1".to_string()));
        assert_eq!(note.liked_count, Some(1430));
        assert_eq!(note.collected_count, Some(12000));
        assert_eq!(note.comment_count, Some(42));
        assert_eq!(note.shared_count, Some(3000));
        assert!(note.publish_time.is_some());
        assert!(note.note_url.is_some());
        assert!(note.creator_profile_url.is_some());
    }

    #[test]
    fn test_note_from_card_skips_none_fields_in_json() {
        let card = NoteCard {
            id: Some("note1".to_string()),
            xsec_token: None,
            xsec_source: None,
            title: "minimal".to_string(),
            ..Default::default()
        };

        let ref_time = chrono::Local::now();
        let note = Note::from_card(&card, None, ref_time);
        let json = serde_json::to_value(&note).unwrap();

        assert!(json.get("xsec_token").is_none());
        assert!(json.get("liked_count").is_none());
        assert!(json.get("creator_name").is_none());
        assert_eq!(json["id"].as_str(), Some("note1"));
        assert_eq!(json["title"].as_str(), Some("minimal"));
    }
}
