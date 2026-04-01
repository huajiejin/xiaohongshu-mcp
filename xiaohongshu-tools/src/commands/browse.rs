use crate::browser::{self, BrowserOptions};
use crate::human::{HumanBehavior, ScrollSpeed};
use anyhow::Result;
use chromiumoxide::element::Element;
use chromiumoxide::page::Page;
use rand::Rng;
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::{info, warn};

const EXPLORE_URL: &str = "https://www.xiaohongshu.com/explore";
const FEED_SECTIONS_SELECTOR: &str = "#exploreFeeds section";
const CLOSE_BTN_SELECTOR: &str = "body > div.note-detail-mask > div.close-circle";

const COMMENT_CONTAINER_SELECTORS: &[&str] = &[
    "#noteContainer div.interaction-container > div.note-scroller",
    "#noteContainer div.interaction-container",
    "#noteContainer",
];

pub struct BrowseOptions {
    pub keywords: Vec<String>,
    pub exclude: Vec<String>,
    pub max_posts: Option<usize>,
    pub scroll_speed: ScrollSpeed,
    pub interact: bool,
    pub duration: Option<u64>,
}

pub async fn run(opts: &BrowseOptions, browser_opts: &BrowserOptions) -> Result<()> {
    let browser = browser::create_browser(browser_opts).await?;
    let page = browser::create_page_with_cookies(&browser, EXPLORE_URL).await?;

    check_login(&page).await;

    let keyword_desc = if opts.keywords.is_empty() {
        "all posts".to_string()
    } else {
        opts.keywords.join(",")
    };
    info!("browsing feed for [{}] (Ctrl+C to stop)", keyword_desc);

    let mut human = HumanBehavior::new();
    let mut seen_hrefs = HashSet::new();
    let mut matched_count = 0usize;
    let start = Instant::now();

    loop {
        if should_stop(opts, matched_count, start) {
            break;
        }

        let sections = match page.find_elements(FEED_SECTIONS_SELECTOR).await {
            Ok(s) => s,
            Err(_) => {
                human.random_delay(human.config.human_delay.clone()).await;
                continue;
            }
        };

        for section in &sections {
            let Some(href) = extract_href(section).await else {
                continue;
            };
            if seen_hrefs.contains(&href) {
                continue;
            }
            seen_hrefs.insert(href.clone());

            let Some(title) = extract_title(section).await else {
                continue;
            };

            if matches_keywords(&title, &opts.exclude) {
                continue;
            }

            if !opts.keywords.is_empty() && !matches_keywords(&title, &opts.keywords) {
                continue;
            }

            matched_count += 1;
            info!("[{matched_count}] {title} | {href}");

            if opts.interact
                && let Err(e) = browse_post(&page, section, &mut human).await
            {
                warn!("browse post failed: {e}");
            }

            if should_stop(opts, matched_count, start) {
                break;
            }
        }

        if should_stop(opts, matched_count, start) {
            break;
        }

        human.scroll_page(&page, opts.scroll_speed).await?;
        human.random_delay(human.config.human_delay.clone()).await;
    }

    info!(
        "done: browsed {} posts in {:?}",
        matched_count,
        start.elapsed()
    );
    Ok(())
}

async fn browse_post(page: &Page, section: &Element, human: &mut HumanBehavior) -> Result<()> {
    if let Err(e) = human.human_click(page, section).await {
        warn!("click post failed: {e}");
        return Err(e);
    }

    human.random_delay(human.config.short_read.clone()).await;

    if let Err(e) = human
        .scroll_container(page, COMMENT_CONTAINER_SELECTORS)
        .await
    {
        warn!("scroll comments failed: {e}");
    }

    human.random_delay(human.config.human_delay.clone()).await;

    close_detail(page).await;

    let mut rng = rand::rng();
    let wait = rng.random_range(1000u64..2000);
    tokio::time::sleep(Duration::from_millis(wait)).await;

    Ok(())
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

fn should_stop(opts: &BrowseOptions, matched: usize, start: Instant) -> bool {
    if let Some(max) = opts.max_posts
        && matched >= max
    {
        info!("reached max posts limit ({max})");
        return true;
    }
    if let Some(dur) = opts.duration
        && start.elapsed().as_secs() >= dur
    {
        info!("reached duration limit ({dur}s)");
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
        warn!("not logged in, feed may be limited. Run 'xhs auth login' first.");
    }
}

async fn extract_href(element: &Element) -> Option<String> {
    let a = element.find_element("a.cover").await.ok()?;
    a.attribute("href").await.ok().flatten()
}

async fn extract_title(element: &Element) -> Option<String> {
    let span = element.find_element("div > div > a > span").await.ok()?;
    span.inner_text().await.ok().flatten()
}

fn matches_keywords(text: &str, keywords: &[String]) -> bool {
    if keywords.is_empty() {
        return false;
    }
    let lower = text.to_lowercase();
    keywords.iter().any(|kw| lower.contains(&kw.to_lowercase()))
}
