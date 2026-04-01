use crate::browser::{self, BrowserOptions};
use crate::cookies;
use crate::login_image;
use anyhow::Result;
use chromiumoxide::page::Page;
use std::time::Duration;
use tracing::{info, warn};

const XHS_URL: &str = "https://www.xiaohongshu.com";
const LOGIN_SELECTOR: &str = ".main-container .user .link-wrapper .channel";

pub async fn login(opts: &BrowserOptions) -> Result<()> {
    info!("Opening browser for login...");

    let browser = browser::create_browser(opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL).await?;

    if is_logged_in(&page).await? {
        info!("Already logged in!");
        let cookies = browser::extract_cookies(&page).await?;
        cookies::save_cookies(&cookies)?;
        return Ok(());
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
                info!("Login successful!");
                let cookies = browser::extract_cookies(&page).await?;
                cookies::save_cookies(&cookies)?;
                info!("Cookies saved");
                break;
            }
            Ok(false) => continue,
            Err(e) => {
                warn!("Error checking login status: {}", e);
                continue;
            }
        }
    }

    Ok(())
}

pub async fn check_status(opts: &BrowserOptions) -> Result<bool> {
    if !cookies::cookies_exist() {
        return Ok(false);
    }

    let browser = browser::create_browser(opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL).await?;

    is_logged_in(&page).await
}

async fn is_logged_in(page: &Page) -> Result<bool> {
    let element = page.find_element(LOGIN_SELECTOR).await;
    Ok(element.is_ok())
}
