use crate::browser::{self, BrowserOptions};
use crate::cookies;
use crate::login_image;
use anyhow::Result;
use chromiumoxide::page::Page;
use serde::Serialize;
use std::fmt;
use std::time::Duration;
use tracing::{debug, info, warn};

const XHS_URL: &str = "https://www.xiaohongshu.com";
const LOGIN_SELECTOR: &str = ".main-container .user .link-wrapper .channel";

#[derive(Serialize)]
pub struct LoginResult {
    pub logged_in: bool,
}

impl fmt::Display for LoginResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Logged in")
    }
}

#[derive(Serialize)]
pub struct StatusResult {
    pub logged_in: bool,
}

impl fmt::Display for StatusResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.logged_in {
            write!(f, "Logged in")
        } else {
            write!(f, "Not logged in")
        }
    }
}

#[derive(Serialize)]
pub struct LogoutResult {
    pub logged_out: bool,
}

impl fmt::Display for LogoutResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Logged out")
    }
}

pub async fn login(opts: &BrowserOptions) -> Result<LoginResult> {
    debug!("Opening browser for login...");

    let browser = browser::create_browser(opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL).await?;

    if is_logged_in(&page).await? {
        debug!("Already logged in!");
        let cookies = browser::extract_cookies(&page).await?;
        cookies::save_cookies(&cookies)?;
        return Ok(LoginResult { logged_in: true });
    }

    if let Err(e) = login_image::fetch_and_open(&page).await {
        warn!("Could not open login image: {e}");
        info!("Please scan the QR code in the browser window...");
    } else {
        info!("\nPlease scan the QR code above to login.\n");
    }

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        match is_logged_in(&page).await {
            Ok(true) => {
                debug!("Login successful!");
                let cookies = browser::extract_cookies(&page).await?;
                cookies::save_cookies(&cookies)?;
                debug!("Cookies saved");
                break;
            }
            Ok(false) => continue,
            Err(e) => {
                warn!("Error checking login status: {}", e);
                continue;
            }
        }
    }

    Ok(LoginResult { logged_in: true })
}

pub async fn check_status(opts: &BrowserOptions) -> Result<StatusResult> {
    if !cookies::cookies_exist() {
        return Ok(StatusResult { logged_in: false });
    }

    let browser = browser::create_browser(opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL).await?;

    let logged_in = is_logged_in(&page).await?;
    Ok(StatusResult { logged_in })
}

async fn is_logged_in(page: &Page) -> Result<bool> {
    let element = page.find_element(LOGIN_SELECTOR).await;
    Ok(element.is_ok())
}
