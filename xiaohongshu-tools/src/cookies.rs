use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const COOKIE_FILE: &str = "cookies.json";

fn get_cookie_path() -> PathBuf {
    if let Ok(path) = std::env::var("COOKIES_PATH") {
        return PathBuf::from(path);
    }
    let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    config_dir.join("xiaohongshu").join(COOKIE_FILE)
}

fn get_cookie_dir() -> PathBuf {
    get_cookie_path().parent().unwrap().to_path_buf()
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

pub fn load_cookies() -> Result<Vec<Cookie>> {
    let path = get_cookie_path();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&path)?;
    let cookies: Vec<Cookie> = serde_json::from_str(&data)?;
    Ok(cookies)
}

pub fn save_cookies(cookies: &[Cookie]) -> Result<()> {
    let dir = get_cookie_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = get_cookie_path();
    let data = serde_json::to_string_pretty(cookies)?;
    fs::write(&path, data)?;
    Ok(())
}

pub fn delete_cookies() -> Result<()> {
    let path = get_cookie_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

pub fn cookies_exist() -> bool {
    get_cookie_path().exists()
}
