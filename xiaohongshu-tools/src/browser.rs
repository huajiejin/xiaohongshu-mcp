use crate::cookies::{self, Cookie};
use anyhow::{Result, anyhow};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures::StreamExt;
use std::time::Duration;

pub async fn create_browser(headless: bool) -> Result<Browser> {
    let mut config = BrowserConfig::builder()
        .request_timeout(Duration::from_secs(60))
        .hide()
        .disable_default_args()
        .arg("no-first-run")
        .arg("no-startup-window")
        .arg("disable-dev-shm-usage")
        .arg("disable-extensions")
        .arg(("disable-features", &["site-per-process", "TranslateUI"][..]))
        .arg("disable-background-networking")
        .arg("disable-background-timer-throttling")
        .arg("disable-backgrounding-occluded-windows")
        .arg("disable-breakpad")
        .arg("disable-client-side-phishing-detection")
        .arg("disable-hang-monitor")
        .arg("disable-prompt-on-repost")
        .arg("disable-sync")
        .arg(("force-color-profile", &["srgb"][..]))
        .arg(("lang", &["en_US"][..]));

    if !headless {
        config = config.with_head();
    }

    let cfg = config
        .build()
        .map_err(|e| anyhow!("Browser config error: {}", e))?;
    let (browser, mut handler) = Browser::launch(cfg).await?;

    tokio::spawn(async move { while handler.next().await.is_some() {} });

    Ok(browser)
}

pub async fn create_page_with_cookies(browser: &Browser, url: &str) -> Result<Page> {
    let page = browser.new_page(url).await?;
    page.enable_stealth_mode().await?;

    let cookies = cookies::load_cookies()?;
    if !cookies.is_empty() {
        let cdp_cookies: Vec<chromiumoxide::cdp::browser_protocol::network::CookieParam> = cookies
            .iter()
            .map(|c| {
                chromiumoxide::cdp::browser_protocol::network::CookieParam::builder()
                    .name(&c.name)
                    .value(&c.value)
                    .domain(c.domain.as_deref().unwrap_or(".xiaohongshu.com"))
                    .path(c.path.as_deref().unwrap_or("/"))
                    .build()
                    .unwrap()
            })
            .collect();
        page.set_cookies(cdp_cookies).await?;
    }

    Ok(page)
}

pub async fn extract_cookies(page: &Page) -> Result<Vec<Cookie>> {
    let cdp_cookies = page.get_cookies().await?;
    let cookies: Vec<Cookie> = cdp_cookies
        .into_iter()
        .map(|c| Cookie {
            name: c.name,
            value: c.value,
            domain: Some(c.domain),
            path: Some(c.path),
            expires: c.expires,
            http_only: c.http_only,
            secure: c.secure,
        })
        .collect();
    Ok(cookies)
}
