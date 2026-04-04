use super::login_image;
use crate::browser::cookies;
use crate::browser::{self, BrowserOptions};
use crate::t;
use anyhow::Result;
use chromiumoxide::page::Page;
use serde::Serialize;
use std::fmt;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

const XHS_URL: &str = "https://www.xiaohongshu.com";
const LOGIN_SELECTOR: &str = ".main-container .user .link-wrapper .channel";

const JS_EXTRACT_USER_INFO: &str = r#"(() => {
    const s = window.__INITIAL_STATE__;
    if (!s) return null;
    const get_val = (o) => o?.value || o?._value || o?._rawValue;
    const info = get_val(s?.user?.userInfo) || s?.user?.userInfo;
    if (!info || typeof info !== 'object') return null;
    return JSON.parse(JSON.stringify(info));
})()"#;

#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub red_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gender: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
}

fn parse_user_info(val: &serde_json::Value) -> Option<UserInfo> {
    let user_id = val
        .get("userId")
        .or_else(|| val.get("user_id"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string)?;

    let gender_num = val.get("gender").and_then(|v| v.as_i64());
    let gender = gender_num.map(|g| match g {
        1 => "male".to_string(),
        2 => "female".to_string(),
        _ => "unknown".to_string(),
    });

    Some(UserInfo {
        user_id,
        nickname: val
            .get("nickname")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        red_id: val
            .get("redId")
            .or_else(|| val.get("red_id"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        desc: val
            .get("desc")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        gender,
        avatar: val
            .get("imageb")
            .and_then(|v| v.as_str())
            .or_else(|| val.get("images").and_then(|v| v.as_str()))
            .map(ToString::to_string),
    })
}

#[derive(Serialize)]
pub struct LoginResult {
    pub logged_in: bool,
}

impl fmt::Display for LoginResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", t!("auth.logged_in"))
    }
}

#[derive(Serialize)]
pub struct StatusResult {
    pub profile: String,
    pub logged_in: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
}

impl fmt::Display for StatusResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.logged_in {
            if let Some(ref u) = self.user {
                write!(
                    f,
                    "{}",
                    t!(
                        "auth.status_logged_in",
                        profile = &self.profile,
                        nickname = u.nickname.as_deref().unwrap_or("-"),
                        red_id = u.red_id.as_deref().unwrap_or("-")
                    )
                )
            } else {
                write!(f, "{}", t!("auth.logged_in"))
            }
        } else {
            write!(
                f,
                "{}",
                t!("auth.status_not_logged_in", profile = &self.profile)
            )
        }
    }
}

#[derive(Serialize)]
pub struct LogoutResult {
    pub logged_out: bool,
}

impl fmt::Display for LogoutResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", t!("auth.logged_out"))
    }
}

pub async fn login(opts: &BrowserOptions, token: &CancellationToken) -> Result<LoginResult> {
    debug!("Opening browser for login...");

    let mut browser = browser::create_browser(opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL, &opts.profile).await?;

    if is_logged_in(&page).await? {
        debug!("Already logged in!");
        let cookies = browser::extract_cookies(&page).await?;
        cookies::save_cookies(&opts.profile, &cookies)?;
        browser.close().await?;
        return Ok(LoginResult { logged_in: true });
    }

    if let Err(e) = login_image::fetch_and_open(&page).await {
        warn!("{}", t!("auth.could_not_open_image", e = e.to_string()));
        info!("{}", t!("auth.scan_qr_browser"));
    } else {
        info!("{}", t!("auth.scan_qr_image"));
    }

    loop {
        tokio::select! {
            _ = token.cancelled() => {
                debug!("cancelled during login wait");
                anyhow::bail!("interrupted");
            }
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
        }

        match is_logged_in(&page).await {
            Ok(true) => {
                debug!("Login successful!");
                let cookies = browser::extract_cookies(&page).await?;
                cookies::save_cookies(&opts.profile, &cookies)?;
                debug!("Cookies saved");
                break;
            }
            Ok(false) => continue,
            Err(e) => {
                warn!("{}", t!("auth.error_checking_status", e = e.to_string()));
                continue;
            }
        }
    }

    browser.close().await?;
    Ok(LoginResult { logged_in: true })
}

pub async fn logout(opts: &BrowserOptions) -> Result<LogoutResult> {
    cookies::delete_cookies(&opts.profile)?;

    let headless_opts = BrowserOptions {
        headless: true,
        proxy: opts.proxy.clone(),
        profile: opts.profile.clone(),
    };
    let mut browser = browser::create_browser(&headless_opts).await?;
    let page = browser.new_page("about:blank").await?;
    page.enable_stealth_mode().await?;
    page.goto(XHS_URL).await?;
    browser::clear_browser_cookies(&page).await?;

    debug!("Logout complete");
    browser.close().await?;
    Ok(LogoutResult { logged_out: true })
}

pub async fn check_status(opts: &BrowserOptions) -> Result<StatusResult> {
    if !cookies::cookies_exist(&opts.profile) {
        return Ok(StatusResult {
            profile: opts.profile.clone(),
            logged_in: false,
            user: None,
        });
    }

    let mut browser = browser::create_browser(opts).await?;
    let page = browser::create_page_with_cookies(&browser, XHS_URL, &opts.profile).await?;

    let logged_in = is_logged_in(&page).await?;

    let user = if logged_in {
        extract_logged_in_user(&page).await
    } else {
        None
    };

    browser.close().await?;
    Ok(StatusResult {
        profile: opts.profile.clone(),
        logged_in,
        user,
    })
}

async fn is_logged_in(page: &Page) -> Result<bool> {
    let element = page.find_element(LOGIN_SELECTOR).await;
    Ok(element.is_ok())
}

async fn extract_logged_in_user(page: &Page) -> Option<UserInfo> {
    let result = page.evaluate(JS_EXTRACT_USER_INFO).await.ok()?;
    let val: serde_json::Value = result.into_value().ok()?;
    if val.is_null() {
        return None;
    }
    parse_user_info(&val)
}
