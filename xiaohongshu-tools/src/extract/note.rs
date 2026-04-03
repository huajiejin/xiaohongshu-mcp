use crate::t;
use anyhow::Result;
use chromiumoxide::page::Page;
use serde::Serialize;

const XHS_DOMAIN: &str = "https://www.xiaohongshu.com";
const EXPLORE_SECTIONS_SELECTOR: &str = "#exploreFeeds section";
const XSEC_SOURCE_PC_SEARCH: &str = "pc_search";
#[allow(dead_code)]
const XSEC_SOURCE_PC_FEED: &str = "pc_feed";
#[allow(dead_code)]
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

    let mut cards = Vec::with_capacity(notes.len());
    for item in notes {
        if let Some(card) = card_from_state_item(item) {
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
        xsec_source: Some("pc_search".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
