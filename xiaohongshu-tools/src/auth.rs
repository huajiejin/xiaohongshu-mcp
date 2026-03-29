use crate::browser::{create_browser, create_page_with_cookies, extract_cookies};
use crate::cookies;
use anyhow::Result;
use chromiumoxide::page::Page;
use std::time::Duration;
use tracing::{info, warn};

const XHS_URL: &str = "https://www.xiaohongshu.com";
const LOGIN_SELECTOR: &str = ".main-container .user .link-wrapper .channel";

pub async fn login() -> Result<()> {
    info!("Opening browser for login...");

    let browser = create_browser(false).await?;
    let page = create_page_with_cookies(&browser, XHS_URL).await?;

    if is_logged_in(&page).await? {
        println!("Already logged in!");
        let cookies = extract_cookies(&page).await?;
        cookies::save_cookies(&cookies)?;
        return Ok(());
    }

    println!("Please scan the QR code to login...");

    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;

        match is_logged_in(&page).await {
            Ok(true) => {
                println!("Login successful!");
                let cookies = extract_cookies(&page).await?;
                cookies::save_cookies(&cookies)?;
                println!("Cookies saved");
                break;
            }
            Ok(false) => {
                continue;
            }
            Err(e) => {
                warn!("Error checking login status: {}", e);
                continue;
            }
        }
    }

    Ok(())
}

pub async fn check_status(headless: bool) -> Result<bool> {
    if !cookies::cookies_exist() {
        return Ok(false);
    }

    let browser = create_browser(headless).await?;
    let page = create_page_with_cookies(&browser, XHS_URL).await?;

    tokio::time::sleep(Duration::from_secs(2)).await;

    is_logged_in(&page).await
}

async fn is_logged_in(page: &Page) -> Result<bool> {
    let element = page.find_element(LOGIN_SELECTOR).await;
    Ok(element.is_ok())
}
