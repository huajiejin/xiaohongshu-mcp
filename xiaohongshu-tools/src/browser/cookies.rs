use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const COOKIE_FILE: &str = "cookies.json";

pub fn get_cookie_path(profile: &str) -> PathBuf {
    if let Ok(path) = std::env::var("COOKIES_PATH") {
        return PathBuf::from(path);
    }
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir
        .join("xiaohongshu")
        .join("profiles")
        .join(profile)
        .join(COOKIE_FILE)
}

fn get_cookie_dir(profile: &str) -> PathBuf {
    let path = get_cookie_path(profile);
    path.parent().unwrap_or_else(|| path.as_ref()).to_path_buf()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: f64,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub secure: bool,
}

pub fn load_cookies(profile: &str) -> Result<Vec<Cookie>> {
    let path = get_cookie_path(profile);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    let cookies: Vec<Cookie> = serde_json::from_str(&data)?;
    Ok(cookies)
}

pub fn save_cookies(profile: &str, cookies: &[Cookie]) -> Result<()> {
    let dir = get_cookie_dir(profile);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = get_cookie_path(profile);
    let data = serde_json::to_string_pretty(cookies)?;
    fs::write(&path, data)?;
    Ok(())
}

pub fn delete_cookies(profile: &str) -> Result<()> {
    let path = get_cookie_path(profile);
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn cookies_exist(profile: &str) -> bool {
    get_cookie_path(profile).exists()
}
