use crate::cookies::{self, Cookie};
use anyhow::{Result, anyhow};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{CookieParam, SetCookiesParams};
use chromiumoxide::page::Page;
use futures::StreamExt;
use rand::Rng;
use std::time::Duration;
use tracing::info;

const COMMON_VIEWPORTS: [(u32, u32); 6] = [
    (1920, 1080),
    (1366, 768),
    (1536, 864),
    (1440, 900),
    (1280, 720),
    (1600, 900),
];

#[derive(Default)]
pub struct BrowserOptions {
    pub headless: bool,
    pub proxy: Option<String>,
}

fn mask_proxy_credentials(url: &str) -> String {
    let Some(at_pos) = url.rfind('@') else {
        return url.to_string();
    };
    let Some(proto_end) = url.find("://") else {
        return url.to_string();
    };
    let proto = &url[..proto_end + 3];
    let rest = &url[at_pos + 1..];
    format!("{proto}***:***@{rest}")
}

pub async fn create_browser(opts: &BrowserOptions) -> Result<Browser> {
    let proxy: Option<String> = opts
        .proxy
        .clone()
        .or_else(|| std::env::var("XHS_PROXY").ok());

    if let Some(ref p) = proxy {
        info!("Using proxy: {}", mask_proxy_credentials(p));
    }

    let mut rng = rand::rng();
    let (w, h) = COMMON_VIEWPORTS[rng.random_range(0..COMMON_VIEWPORTS.len())];
    info!("Viewport: {}x{}", w, h);

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
        .arg("disable-infobars")
        .arg("disable-notifications")
        .arg("disable-blink-features=AutomationControlled")
        .arg(("force-color-profile", &["srgb"][..]))
        .arg(("lang", &["zh-CN"][..]))
        .arg(format!("window-size={},{}", w, h));

    if let Some(proxy_url) = proxy {
        let url_owned = proxy_url.to_string();
        config = config.arg(format!("proxy-server={}", url_owned));
    }

    if !opts.headless {
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
    let page = browser.new_page("about:blank").await?;
    page.enable_stealth_mode().await?;

    let cookies = cookies::load_cookies()?;
    if !cookies.is_empty() {
        let cdp_cookies: Vec<CookieParam> = cookies
            .iter()
            .map(|c| {
                CookieParam::builder()
                    .name(&c.name)
                    .value(&c.value)
                    .domain(c.domain.as_deref().unwrap_or(".xiaohongshu.com"))
                    .path(c.path.as_deref().unwrap_or("/"))
                    .build()
                    .unwrap()
            })
            .collect();
        page.execute(SetCookiesParams::new(cdp_cookies)).await?;
    }

    page.goto(url).await?;

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
