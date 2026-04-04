use super::cookies::{self, Cookie};
use crate::{browser, t};
use anyhow::{Result, anyhow};
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::{
    ClearBrowserCookiesParams, CookieParam, SetCookiesParams,
};
use chromiumoxide::page::Page;
use futures::StreamExt;
use rand::Rng;
use std::ops::Deref;
use std::path::PathBuf;
use std::time::Duration;
use tracing::debug;

const COMMON_VIEWPORTS: [(u32, u32); 6] = [
    (1920, 1080),
    (1366, 768),
    (1536, 864),
    (1440, 900),
    (1280, 720),
    (1600, 900),
];

pub struct BrowserOptions {
    pub headless: bool,
    pub proxy: Option<String>,
    pub profile: String,
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

pub struct BrowserHandle {
    browser: Browser,
    user_data_dir: PathBuf,
}

impl Deref for BrowserHandle {
    type Target = Browser;
    fn deref(&self) -> &Self::Target {
        &self.browser
    }
}

impl BrowserHandle {
    pub async fn close(&mut self) -> Result<()> {
        self.browser.close().await?;
        self.cleanup_dir();
        Ok(())
    }

    fn cleanup_dir(&self) {
        if self.user_data_dir.exists()
            && let Err(e) = std::fs::remove_dir_all(&self.user_data_dir)
        {
            debug!("failed to remove user-data-dir: {e}");
        }
    }
}

impl Drop for BrowserHandle {
    fn drop(&mut self) {
        if let Some(child) = self.browser.get_mut_child()
            && let Some(id) = child.as_mut_inner().id()
        {
            let _ = std::process::Command::new("kill")
                .args(["-9", &id.to_string()])
                .output();
        }
        self.cleanup_dir();
    }
}

pub async fn create_browser(opts: &BrowserOptions) -> Result<BrowserHandle> {
    let proxy: Option<String> = opts
        .proxy
        .clone()
        .or_else(|| std::env::var("XHS_PROXY").ok());

    if let Some(ref p) = proxy {
        debug!("Using proxy: {}", mask_proxy_credentials(p));
    }

    let (w, h) = {
        let mut rng = rand::rng();
        let idx = rng.random_range(0..COMMON_VIEWPORTS.len());
        COMMON_VIEWPORTS
            .get(idx)
            .copied()
            .expect("viewport index in bounds")
    };
    debug!("Viewport: {}x{}", w, h);

    let max_attempts = 3;
    for attempt in 0..max_attempts {
        let (cfg, user_data_dir) = build_config(w, h, &proxy, opts.headless)?;
        match Browser::launch(cfg).await {
            Ok((browser, mut handler)) => {
                tokio::spawn(async move { while handler.next().await.is_some() {} });
                return Ok(BrowserHandle {
                    browser,
                    user_data_dir,
                });
            }
            Err(e) if attempt + 1 < max_attempts => {
                debug!(attempt, error = %e, "browser launch failed, retrying");
            }
            Err(e) => return Err(e.into()),
        }
    }

    unreachable!()
}

fn build_config(
    w: u32,
    h: u32,
    proxy: &Option<String>,
    headless: bool,
) -> Result<(chromiumoxide::browser::BrowserConfig, PathBuf)> {
    let port = {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        listener.local_addr()?.port()
    };
    let user_data_dir = std::env::temp_dir().join(format!("xhs-browser-{}", port));
    debug!("Browser port: {}, user-data-dir: {:?}", port, user_data_dir);

    let mut config = BrowserConfig::builder()
        .port(port)
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
        .arg(format!("window-size={},{}", w, h))
        .arg(format!("user-data-dir={}", user_data_dir.display()));

    if let Some(proxy_url) = proxy {
        config = config.arg(format!("proxy-server={}", proxy_url));
    }

    if !headless {
        config = config.with_head();
    }

    config
        .build()
        .map_err(|e| anyhow!("{}", t!("browser.config_error", e = e)))
        .map(|c| (c, user_data_dir))
}

pub async fn create_page_with_cookies(browser: &Browser, url: &str, profile: &str) -> Result<Page> {
    let page = browser.new_page("about:blank").await?;
    page.enable_stealth_mode().await?;

    browser::clear_browser_cookies(&page).await?;

    let cookies = cookies::load_cookies(profile)?;
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
                    .map_err(|e| anyhow::anyhow!("{}", t!("browser.cookie_param_error", e = e)))
            })
            .collect::<Result<_>>()?;
        page.execute(SetCookiesParams::new(cdp_cookies)).await?;
    }

    page.goto(url).await?;

    Ok(page)
}

pub async fn clear_browser_cookies(page: &Page) -> Result<()> {
    page.execute(ClearBrowserCookiesParams::default()).await?;
    debug!("Browser cookies cleared");
    Ok(())
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
