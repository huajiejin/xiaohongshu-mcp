use crate::browser::{self, BrowserOptions};
use crate::cookies;
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
        println!("Already logged in!");
        let cookies = browser::extract_cookies(&page).await?;
        cookies::save_cookies(&cookies)?;
        return Ok(());
    }

    println!("Please scan the QR code to login...");

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        match is_logged_in(&page).await {
            Ok(true) => {
                println!("Login successful!");
                let cookies = browser::extract_cookies(&page).await?;
                cookies::save_cookies(&cookies)?;
                println!("Cookies saved");
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

    tokio::time::sleep(Duration::from_secs(2)).await;

    is_logged_in(&page).await
}

async fn is_logged_in(page: &Page) -> Result<bool> {
    let element = page.find_element(LOGIN_SELECTOR).await;
    Ok(element.is_ok())
}
