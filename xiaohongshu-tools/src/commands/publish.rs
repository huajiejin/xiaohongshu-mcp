use crate::browser::human::HumanBehavior;
use crate::browser::{self, BrowserOptions};
use crate::selectors::Selector;
use crate::shared::file_input::wait_for_file_inputs;
use crate::shared::utils::poll_until;
use crate::t;
use anyhow::{Result, anyhow, bail};
use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
use chromiumoxide::page::Page;
use serde::Serialize;
use std::fmt;
use std::path::Path;
use std::time::Duration;
use tracing::debug;

#[allow(dead_code)]
const CREATOR_PUBLISH_NORMAL_URL: &str =
    "https://creator.xiaohongshu.com/publish/publish?source=official&from=tab_switch&target=image";
#[allow(dead_code)]
const CREATOR_PUBLISH_VIDEO_URL: &str =
    "https://creator.xiaohongshu.com/publish/publish?source=official&from=tab_switch&target=video";
const XHS_URL: &str = "https://www.xiaohongshu.com";
const MAX_TAGS: usize = 10;
const TITLE_MAX_UTF16_LEN: usize = 20;

pub struct PublishNormalOptions {
    pub title: String,
    pub content: String,
    pub images: Vec<String>,
    pub tags: Vec<String>,
    pub schedule: Option<String>,
    pub visibility: String,
    pub is_original: bool,
    pub draft: bool,
}

#[derive(Serialize)]
pub struct PublishResult {
    pub action: String,
    pub success: bool,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<bool>,
    pub scheduled_at: Option<String>,
    pub visibility: String,
    pub is_original: bool,
    pub is_draft: bool,
}

impl fmt::Display for PublishResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let action = if self.is_draft { "draft" } else { "publish" };
        if self.video == Some(true) {
            writeln!(
                f,
                "{}",
                t!("publish.done_video", action = action, title = &self.title,)
            )?;
        } else {
            let image_count = self.image_count.unwrap_or(0);
            writeln!(
                f,
                "{}",
                t!(
                    "publish.done",
                    action = action,
                    title = &self.title,
                    image_count = image_count,
                )
            )?;
        }
        if let Some(ref scheduled) = self.scheduled_at {
            writeln!(
                f,
                "{}",
                t!("publish.scheduled_at", scheduled_at = scheduled)
            )?;
        }
        Ok(())
    }
}

pub async fn run(
    opts: &PublishNormalOptions,
    browser_opts: &BrowserOptions,
) -> Result<PublishResult> {
    validate_inputs(opts)?;

    let action = if opts.draft { "draft" } else { "publish" };
    debug!("{action}: title={}", opts.title);

    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL, &browser_opts.profile).await?;

    let mut human = HumanBehavior::new();

    let creator_page = click_explore_page_publish_button(&browser, &page, &mut human).await?;

    wait_creator_page(&creator_page).await?;

    click_publish_tab(&creator_page, "上传图文", &mut human).await?;
    human.random_delay(human.config.human_delay.clone()).await;

    upload_images(&creator_page, &opts.images, &mut human).await?;

    fill_title(&creator_page, &opts.title, &mut human).await?;
    fill_content(&creator_page, &opts.content, &opts.tags, &mut human).await?;

    if opts.is_original {
        set_original(&creator_page, &mut human).await?;
    }

    set_visibility(&creator_page, &opts.visibility, &mut human).await?;

    if let Some(ref schedule) = opts.schedule {
        set_schedule(&creator_page, schedule, &mut human).await?;
    }

    let btn_text = if opts.draft {
        "暂存离开"
    } else if opts.schedule.is_some() {
        "定时发布"
    } else {
        "发布"
    };
    click_action_button(&creator_page, btn_text, &mut human).await?;

    human.random_delay(human.config.read_time.clone()).await;

    Ok(PublishResult {
        action: action.to_string(),
        success: true,
        title: opts.title.clone(),
        image_count: Some(opts.images.len()),
        video: None,
        scheduled_at: opts.schedule.clone(),
        visibility: opts.visibility.clone(),
        is_original: opts.is_original,
        is_draft: opts.draft,
    })
}

const VIDEO_UPLOAD_TIMEOUT_SECS: u64 = 300;

pub struct PublishVideoOptions {
    pub title: String,
    pub content: String,
    pub video: String,
    pub cover: Option<String>,
    pub tags: Vec<String>,
    pub schedule: Option<String>,
    pub visibility: String,
    pub is_original: bool,
    pub draft: bool,
}

pub async fn run_video(
    opts: &PublishVideoOptions,
    browser_opts: &BrowserOptions,
) -> Result<PublishResult> {
    validate_video_inputs(opts)?;

    let action = if opts.draft { "draft" } else { "publish" };
    debug!("{action} video: title={}", opts.title);

    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL, &browser_opts.profile).await?;

    let mut human = HumanBehavior::new();

    let creator_page = click_explore_page_publish_button(&browser, &page, &mut human).await?;

    wait_creator_page(&creator_page).await?;

    click_publish_tab(&creator_page, "上传视频", &mut human).await?;
    human.random_delay(human.config.human_delay.clone()).await;

    upload_video(&creator_page, &opts.video, &mut human).await?;

    if let Some(ref cover) = opts.cover {
        upload_cover(&creator_page, cover, &mut human).await?;
    }

    fill_title(&creator_page, &opts.title, &mut human).await?;
    fill_content(&creator_page, &opts.content, &opts.tags, &mut human).await?;

    if opts.is_original {
        set_original(&creator_page, &mut human).await?;
    }

    set_visibility(&creator_page, &opts.visibility, &mut human).await?;

    if let Some(ref schedule) = opts.schedule {
        set_schedule(&creator_page, schedule, &mut human).await?;
    }

    let btn_text = if opts.draft {
        "暂存离开"
    } else if opts.schedule.is_some() {
        "定时发布"
    } else {
        "发布"
    };
    click_action_button(&creator_page, btn_text, &mut human).await?;

    human.random_delay(human.config.read_time.clone()).await;

    Ok(PublishResult {
        action: action.to_string(),
        success: true,
        title: opts.title.clone(),
        image_count: None,
        video: Some(true),
        scheduled_at: opts.schedule.clone(),
        visibility: opts.visibility.clone(),
        is_original: opts.is_original,
        is_draft: opts.draft,
    })
}

fn validate_video_inputs(opts: &PublishVideoOptions) -> Result<()> {
    if opts.title.is_empty() {
        bail!("{}", t!("publish.title_empty"));
    }

    let utf16_len = calc_utf16_title_len(&opts.title);
    if utf16_len > TITLE_MAX_UTF16_LEN {
        bail!(
            "{}",
            t!(
                "publish.title_too_long",
                current = utf16_len,
                max = TITLE_MAX_UTF16_LEN,
            )
        );
    }

    if opts.video.is_empty() {
        bail!("{}", t!("publish.video_empty"));
    }

    if !Path::new(&opts.video).exists() {
        bail!("{}", t!("publish.video_not_found", path = &opts.video));
    }

    if let Some(ref cover) = opts.cover
        && !Path::new(cover).exists()
    {
        bail!("{}", t!("publish.cover_not_found", path = cover));
    }

    if let Some(ref schedule) = opts.schedule {
        let dt = chrono::NaiveDateTime::parse_from_str(schedule, "%Y-%m-%dT%H:%M")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(schedule, "%Y-%m-%d %H:%M"))
            .map_err(|_| anyhow!("{}", t!("publish.invalid_schedule")))?;

        let scheduled = dt.and_utc();
        let min = chrono::Utc::now() + chrono::Duration::hours(1);
        let max = chrono::Utc::now() + chrono::Duration::days(14);

        if scheduled < min {
            bail!("{}", t!("publish.schedule_too_soon"));
        }
        if scheduled > max {
            bail!("{}", t!("publish.schedule_too_far"));
        }
    }

    Ok(())
}

async fn upload_video(page: &Page, video_path: &str, human: &mut HumanBehavior) -> Result<()> {
    let file_inputs = wait_for_file_inputs(page, 10, |inputs| !inputs.videos.is_empty())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!("publish.upload_input_not_found", error = e.to_string())
            )
        })?;

    let file_input = file_inputs
        .videos
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no video file input found on page"))?;

    let node_id = file_input
        .description()
        .await
        .map(|d| d.node_id)
        .map_err(|e| anyhow!("{}", t!("publish.upload_node_error", error = e.to_string())))?;

    let params = SetFileInputFilesParams::builder()
        .files(vec![video_path.to_string()])
        .node_id(node_id)
        .build()
        .map_err(|e| anyhow!("SetFileInputFiles build: {e}"))?;

    page.execute(params).await?;

    debug!("video upload started: {video_path}");

    poll_until(
        Duration::from_secs(VIDEO_UPLOAD_TIMEOUT_SECS),
        Duration::from_secs(2),
        || async {
            let js = format!(
                r#"(() => {{
                    const progress = {};
                    if (progress) {{
                        const text = progress.textContent || '';
                        if (text.includes('100%') || text.includes('上传完成') || text.includes('Upload complete')) {{
                            return 'done';
                        }}
                        return text;
                    }}
                    const preview = document.querySelector('{}');
                    if (preview) return 'done';
                    return 'uploading';
                }})()"#,
                Selector::VideoUploadProgress.js_query(),
                Selector::VideoPreview.css(),
            );
            let result = page
                .evaluate_expression(js)
                .await
                .ok()
                .and_then(|v| v.into_value::<String>().ok())
                .unwrap_or_default();
            if result == "done" {
                Some(())
            } else {
                debug!("video upload progress: {result}");
                None
            }
        },
    )
    .await
    .map_err(|_| {
        anyhow!(
            "{}",
            t!("publish.video_upload_timeout", timeout = VIDEO_UPLOAD_TIMEOUT_SECS)
        )
    })?;

    human.random_delay(human.config.human_delay.clone()).await;
    debug!("video uploaded successfully");
    Ok(())
}

async fn upload_cover(page: &Page, cover_path: &str, human: &mut HumanBehavior) -> Result<()> {
    let cover_el = poll_until(
        Duration::from_secs(10),
        Duration::from_millis(500),
        || async { page.find_element(Selector::CoverDefault.css()).await.ok() },
    )
    .await
    .map_err(|_| anyhow!("{}", t!("publish.cover_modal_not_found")))?;

    human.human_click(page, &cover_el).await?;
    debug!("cover edit modal opened");
    human.random_delay(human.config.human_delay.clone()).await;

    let file_inputs = wait_for_file_inputs(page, 10, |inputs| !inputs.images.is_empty())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!("publish.upload_input_not_found", error = e.to_string())
            )
        })?;

    let file_input = file_inputs
        .images
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("no image file input found for cover upload"))?;

    let node_id = file_input
        .description()
        .await
        .map(|d| d.node_id)
        .map_err(|e| anyhow!("{}", t!("publish.upload_node_error", error = e.to_string())))?;

    let params = SetFileInputFilesParams::builder()
        .files(vec![cover_path.to_string()])
        .node_id(node_id)
        .build()
        .map_err(|e| anyhow!("SetFileInputFiles build: {e}"))?;

    page.execute(params).await?;
    debug!("cover file set: {cover_path}");

    human
        .random_delay(Duration::from_millis(800)..Duration::from_millis(1200))
        .await;

    let confirm_btn = poll_until(
        Duration::from_secs(10),
        Duration::from_millis(500),
        || async {
            let buttons = page
                .find_elements(Selector::CoverConfirmBtn.css())
                .await
                .unwrap_or_default();
            for btn in buttons {
                let text = btn.inner_text().await.ok().flatten().unwrap_or_default();
                if text.trim() == "确定" {
                    return Some(btn);
                }
            }
            None
        },
    )
    .await
    .map_err(|_| anyhow!("{}", t!("publish.cover_confirm_not_found")))?;

    human.human_click(page, &confirm_btn).await?;
    debug!("cover confirm clicked");

    poll_until(
        Duration::from_secs(10),
        Duration::from_millis(500),
        || async {
            let modal_visible = page.find_element(Selector::ModalMask.css()).await.is_ok();
            if modal_visible { None } else { Some(()) }
        },
    )
    .await
    .map_err(|_| anyhow!("{}", t!("publish.cover_upload_timeout")))?;

    human.random_delay(human.config.human_delay.clone()).await;
    debug!("cover uploaded successfully");
    Ok(())
}

fn validate_inputs(opts: &PublishNormalOptions) -> Result<()> {
    if opts.title.is_empty() {
        bail!("{}", t!("publish.title_empty"));
    }

    let utf16_len = calc_utf16_title_len(&opts.title);
    if utf16_len > TITLE_MAX_UTF16_LEN {
        bail!(
            "{}",
            t!(
                "publish.title_too_long",
                current = utf16_len,
                max = TITLE_MAX_UTF16_LEN,
            )
        );
    }

    if opts.images.is_empty() {
        bail!("{}", t!("publish.images_empty"));
    }

    for path in &opts.images {
        if !Path::new(path).exists() {
            bail!("{}", t!("publish.image_not_found", path = path));
        }
    }

    if let Some(ref schedule) = opts.schedule {
        let dt = chrono::NaiveDateTime::parse_from_str(schedule, "%Y-%m-%dT%H:%M")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(schedule, "%Y-%m-%d %H:%M"))
            .map_err(|_| anyhow!("{}", t!("publish.invalid_schedule")))?;

        let scheduled = dt.and_utc();
        let min = chrono::Utc::now() + chrono::Duration::hours(1);
        let max = chrono::Utc::now() + chrono::Duration::days(14);

        if scheduled < min {
            bail!("{}", t!("publish.schedule_too_soon"));
        }
        if scheduled > max {
            bail!("{}", t!("publish.schedule_too_far"));
        }
    }

    Ok(())
}

fn calc_utf16_title_len(title: &str) -> usize {
    title.encode_utf16().count()
}

async fn wait_creator_page(page: &Page) -> Result<()> {
    poll_until(
        Duration::from_secs(30),
        Duration::from_millis(500),
        || async {
            let js = format!(
                r#"(() => {{
                    const el = {};
                    return el && el.offsetParent !== null;
                }})()"#,
                Selector::UploadContent.js_query()
            );
            page.evaluate_expression(js)
                .await
                .ok()
                .and_then(|v| v.into_value::<serde_json::Value>().ok())
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                .then_some(())
        },
    )
    .await
    .map_err(|_| anyhow!("{}", t!("publish.page_timeout")))?;

    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(())
}

async fn click_publish_tab(page: &Page, tab_name: &str, human: &mut HumanBehavior) -> Result<()> {
    let el = poll_until(
        Duration::from_secs(15),
        Duration::from_millis(300),
        || async {
            remove_popover(page).await;

            let elements = page
                .find_elements(Selector::CreatorTab.css())
                .await
                .unwrap_or_default();

            let mut found = None;
            for el in elements {
                let text = el.inner_text().await.ok().flatten().unwrap_or_default();
                if text.trim() == tab_name {
                    found = Some(el);
                    break;
                }
            }
            found
        },
    )
    .await
    .map_err(|_| anyhow!("{}", t!("publish.tab_not_found", tab = tab_name)))?;

    human.human_click(page, &el).await?;
    debug!("clicked tab: {tab_name}");
    Ok(())
}

async fn remove_popover(page: &Page) {
    let js = format!(
        r#"(() => {{
            const pop = {};
            if (pop) pop.remove();
        }})()"#,
        Selector::Popover.js_query()
    );
    let _ = page.evaluate_expression(js).await;
}

async fn upload_images(page: &Page, images: &[String], human: &mut HumanBehavior) -> Result<()> {
    let mut valid_paths: Vec<String> = Vec::new();
    for path in images {
        if Path::new(path).exists() {
            valid_paths.push(path.clone());
            debug!("valid image: {path}");
        } else {
            debug!("skipping missing image: {path}");
        }
    }

    if valid_paths.is_empty() {
        bail!("{}", t!("publish.images_empty"));
    }

    for (i, path) in valid_paths.iter().enumerate() {
        let file_inputs = wait_for_file_inputs(page, 10, |inputs| !inputs.images.is_empty())
            .await
            .map_err(|e| {
                anyhow!(
                    "{}",
                    t!("publish.upload_input_not_found", error = e.to_string())
                )
            })?;

        let file_input = file_inputs
            .images
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("no image file input found on page"))?;

        let node_id = file_input
            .description()
            .await
            .map(|d| d.node_id)
            .map_err(|e| anyhow!("{}", t!("publish.upload_node_error", error = e.to_string())))?;

        let params = SetFileInputFilesParams::builder()
            .files(vec![path.clone()])
            .node_id(node_id)
            .build()
            .map_err(|e| anyhow!("SetFileInputFiles build: {e}"))?;

        page.execute(params).await?;

        debug!(
            "uploaded image {}/{}, waiting for preview",
            i + 1,
            valid_paths.len()
        );

        let expected = i + 1;
        poll_until(
            Duration::from_secs(60),
            Duration::from_millis(500),
            || async {
                let count = page
                    .find_elements(Selector::ImagePreview.css())
                    .await
                    .map(|els| els.len())
                    .unwrap_or(0);
                if count >= expected { Some(()) } else { None }
            },
        )
        .await
        .map_err(|_| anyhow!("{}", t!("publish.upload_timeout", index = i + 1)))?;

        human.random_delay(human.config.human_delay.clone()).await;
    }

    debug!("all {} images uploaded", valid_paths.len());
    Ok(())
}

async fn fill_title(page: &Page, title: &str, human: &mut HumanBehavior) -> Result<()> {
    let input = page
        .find_element(Selector::TitleInput.css())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!("publish.title_input_not_found", error = e.to_string())
            )
        })?;

    human.human_type_text(page, &input, title).await?;

    tokio::time::sleep(Duration::from_millis(500)).await;

    let has_max_indicator = page
        .find_element(Selector::TitleMaxSuffix.css())
        .await
        .is_ok();

    if has_max_indicator {
        bail!(
            "{}",
            t!(
                "publish.title_too_long",
                current = calc_utf16_title_len(title),
                max = TITLE_MAX_UTF16_LEN
            )
        );
    }

    human.random_delay(human.config.human_delay.clone()).await;
    Ok(())
}

async fn fill_content(
    page: &Page,
    content: &str,
    tags: &[String],
    human: &mut HumanBehavior,
) -> Result<()> {
    let content_el = if let Ok(el) = page.find_element(Selector::EditorContent.css()).await {
        el
    } else if let Ok(el) = page.find_element(Selector::EditorEmpty.css()).await {
        el
    } else {
        bail!("{}", t!("publish.content_input_not_found"));
    };

    human.human_type_text(page, &content_el, content).await?;

    if !tags.is_empty() {
        let tags_to_use = if tags.len() > MAX_TAGS {
            debug!("truncating {} tags to {}", tags.len(), MAX_TAGS);
            &tags[..MAX_TAGS]
        } else {
            tags
        };
        input_tags(page, &content_el, tags_to_use, human).await?;
    }

    let title_input = page.find_element(Selector::TitleInput.css()).await.ok();
    if let Some(title_el) = title_input {
        human.human_click(page, &title_el).await?;
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    let has_length_error = page
        .find_element(Selector::ContentLengthError.css())
        .await
        .is_ok();

    if has_length_error {
        bail!("{}", t!("publish.content_too_long"));
    }

    Ok(())
}

async fn input_tags(
    page: &Page,
    content_el: &chromiumoxide::element::Element,
    tags: &[String],
    human: &mut HumanBehavior,
) -> Result<()> {
    human.random_delay(human.config.human_delay.clone()).await;

    for _ in 0..20 {
        let _ = content_el
            .call_js_fn("function() { this.dispatchEvent(new KeyboardEvent('keydown', {key:'ArrowDown',keyCode:40,bubbles:true})); }", false)
            .await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let _ = content_el
        .call_js_fn("function() { this.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',keyCode:13,bubbles:true})); }", false)
        .await;
    let _ = content_el
        .call_js_fn("function() { this.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter',keyCode:13,bubbles:true})); }", false)
        .await;

    tokio::time::sleep(Duration::from_secs(1)).await;

    for tag in tags {
        let tag = tag.trim_start_matches('#').to_string();
        if tag.is_empty() {
            continue;
        }

        page.execute(chromiumoxide::cdp::browser_protocol::input::InsertTextParams::new("#"))
            .await?;
        tokio::time::sleep(Duration::from_millis(200)).await;

        for ch in tag.chars() {
            let ch_str = ch.to_string();
            page.execute(
                chromiumoxide::cdp::browser_protocol::input::InsertTextParams::new(&ch_str),
            )
            .await?;
            human
                .random_delay(Duration::from_millis(30)..Duration::from_millis(80))
                .await;
        }

        tokio::time::sleep(Duration::from_secs(1)).await;

        let topic_found = page.find_element(Selector::TopicItem.css()).await;

        if let Ok(item) = topic_found {
            human.human_click(page, &item).await?;
            debug!("tag '{tag}': clicked autocomplete suggestion");
        } else {
            debug!("tag '{tag}': no autocomplete, typing space");
            page.execute(chromiumoxide::cdp::browser_protocol::input::InsertTextParams::new(" "))
                .await?;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    Ok(())
}

async fn set_schedule(page: &Page, schedule: &str, human: &mut HumanBehavior) -> Result<()> {
    let switch = page
        .find_element(Selector::ScheduleSwitch.css())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!("publish.schedule_switch_not_found", error = e.to_string())
            )
        })?;

    human.human_click(page, &switch).await?;
    human
        .random_delay(Duration::from_millis(800)..Duration::from_millis(1200))
        .await;

    let input = page
        .find_element(Selector::DatePickerInput.css())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!("publish.date_input_not_found", error = e.to_string())
            )
        })?;

    let dt = chrono::NaiveDateTime::parse_from_str(schedule, "%Y-%m-%dT%H:%M")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(schedule, "%Y-%m-%d %H:%M"))
        .map_err(|_| anyhow!("{}", t!("publish.invalid_schedule")))?;

    let formatted = dt.format("%Y-%m-%d %H:%M").to_string();
    human
        .clear_and_input(page, &input, Some(&formatted))
        .await?;

    human
        .random_delay(Duration::from_millis(500)..Duration::from_millis(800))
        .await;
    debug!("schedule set to: {formatted}");
    Ok(())
}

async fn set_visibility(page: &Page, visibility: &str, human: &mut HumanBehavior) -> Result<()> {
    let vis_str = match visibility {
        "public" => "公开可见",
        "private" => "仅自己可见",
        "friends" => "仅互关好友可见",
        other => other,
    };

    if vis_str == "公开可见" || vis_str.is_empty() {
        return Ok(());
    }

    let dropdown = page
        .find_element(Selector::VisibilityDropdown.css())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!(
                    "publish.visibility_dropdown_not_found",
                    error = e.to_string()
                )
            )
        })?;

    human.human_click(page, &dropdown).await?;
    human
        .random_delay(Duration::from_millis(400)..Duration::from_millis(600))
        .await;

    let options = page
        .find_elements(Selector::VisibilityOption.css())
        .await
        .unwrap_or_default();

    for opt in &options {
        let text = opt.inner_text().await.ok().flatten().unwrap_or_default();
        if text.contains(vis_str) {
            human.human_click(page, opt).await?;
            debug!("visibility set to: {vis_str}");
            return Ok(());
        }
    }

    bail!(
        "{}",
        t!("publish.visibility_option_not_found", visibility = vis_str)
    );
}

async fn set_original(page: &Page, human: &mut HumanBehavior) -> Result<()> {
    let cards = page
        .find_elements(Selector::OriginalSwitchCard.css())
        .await
        .unwrap_or_default();

    for card in &cards {
        let text = card.inner_text().await.ok().flatten().unwrap_or_default();
        if !text.contains("原创声明") {
            continue;
        }

        let switch_el = card
            .find_element(Selector::OriginalSwitch.css())
            .await
            .map_err(|_| anyhow!("{}", t!("publish.original_switch_not_found")))?;

        let already_on = switch_el
            .call_js_fn(
                r#"function() { const input = this.querySelector('input[type="checkbox"]'); return input ? input.checked : false; }"#,
                false,
            )
            .await
            .ok()
            .and_then(|ret| ret.result.value)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if already_on {
            debug!("original declaration already enabled");
            return Ok(());
        }

        human.human_click(page, &switch_el).await?;
        human
            .random_delay(Duration::from_millis(500)..Duration::from_millis(800))
            .await;

        confirm_original_dialog(page, human).await?;
        debug!("original declaration enabled");
        return Ok(());
    }

    bail!("{}", t!("publish.original_card_not_found"));
}

async fn confirm_original_dialog(page: &Page, human: &mut HumanBehavior) -> Result<()> {
    tokio::time::sleep(Duration::from_millis(800)).await;

    let js = r#"(() => {
        const footers = document.querySelectorAll('div.footer');
        for (const footer of footers) {
            if (!footer.textContent.includes('原创声明须知')) continue;
            const checkbox = footer.querySelector('div.d-checkbox input[type="checkbox"]');
            if (checkbox && !checkbox.checked) {
                checkbox.click();
            }
            return 'checked';
        }
        return 'not_found';
    })()"#;

    let _ = page.evaluate_expression(js).await;
    human
        .random_delay(Duration::from_millis(500)..Duration::from_millis(800))
        .await;

    let js2 = r#"(() => {
        const footers = document.querySelectorAll('div.footer');
        for (const footer of footers) {
            if (!footer.textContent.includes('声明原创')) continue;
            const btn = footer.querySelector('button.custom-button');
            if (!btn) return 'button_not_found';
            if (btn.classList.contains('disabled') || btn.disabled) {
                const checkbox = footer.querySelector('div.d-checkbox input[type="checkbox"]');
                if (checkbox && !checkbox.checked) checkbox.click();
                return 'button_disabled';
            }
            btn.click();
            return 'clicked';
        }
        return 'not_found';
    })()"#;

    let result = page
        .evaluate_expression(js2)
        .await
        .ok()
        .and_then(|v| v.into_value::<String>().ok())
        .unwrap_or_default();

    match result.as_str() {
        "clicked" => {
            debug!("original declaration confirmed");
            Ok(())
        }
        "button_disabled" => bail!("{}", t!("publish.original_button_disabled")),
        _ => bail!("{}", t!("publish.original_dialog_not_found")),
    }
}

async fn click_explore_page_publish_button(
    browser: &Browser,
    page: &Page,
    human: &mut HumanBehavior,
) -> Result<Page> {
    let pages_before = browser.pages().await?;
    let count_before = pages_before.len();

    let btn = page
        .find_element(Selector::ExplorePagePublishBtn.css())
        .await
        .map_err(|e| {
            anyhow!(
                "{}",
                t!(
                    "publish.explore_page_publish_btn_not_found",
                    error = e.to_string()
                )
            )
        })?;

    human.human_click(page, &btn).await?;
    debug!("[explore page] publish button clicked");

    let new_page = poll_until(
        Duration::from_secs(15),
        Duration::from_millis(500),
        || async {
            let all = browser.pages().await.unwrap_or_default();
            if all.len() <= count_before {
                return None;
            }
            let mut result = None;
            for p in all {
                if let Some(u) = p.url().await.ok().flatten()
                    && u.contains("creator.xiaohongshu.com")
                {
                    result = Some(p);
                    break;
                }
            }
            result
        },
    )
    .await
    .map_err(|_| anyhow!("timed out waiting for creator page to open in new tab"))?;

    debug!("[explore page] switched to creator tab");
    Ok(new_page)
}

async fn click_action_button(
    page: &Page,
    button_text: &str,
    human: &mut HumanBehavior,
) -> Result<()> {
    let el = poll_until(
        Duration::from_secs(15),
        Duration::from_millis(500),
        || async {
            let buttons = page
                .find_elements(Selector::ActionButton.css())
                .await
                .unwrap_or_default();

            let mut found = None;
            for btn in buttons {
                let text = btn.inner_text().await.ok().flatten().unwrap_or_default();
                if text.trim() == button_text {
                    found = Some(btn);
                    break;
                }
            }
            found
        },
    )
    .await
    .map_err(|_| {
        anyhow!(
            "{}",
            t!("publish.publish_btn_not_found", text = button_text)
        )
    })?;

    human.human_click(page, &el).await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    debug!("{button_text} button clicked");
    Ok(())
}
