use anyhow::{Result, anyhow, bail};
use base64::Engine;
use chromiumoxide::page::Page;
use std::env;
use std::fs;
use std::process::Command;
use std::time::{Duration, Instant};
use tracing::info;

const IMAGE_SELECTOR: &str = "#app > div:nth-child(1) > div > div.login-container > div.container > div.code-area > div.qrcode.force-light > img.qrcode-img";

const WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const IMAGE_FILENAME: &str = "xhs-login.png";

pub async fn fetch_and_open(page: &Page) -> Result<()> {
    let data = fetch_image_bytes(page).await?;
    save_and_open(&data)
}

async fn fetch_image_bytes(page: &Page) -> Result<Vec<u8>> {
    let deadline = Instant::now() + WAIT_TIMEOUT;

    loop {
        if let Ok(element) = page.find_element(IMAGE_SELECTOR).await
            && let Ok(Some(src)) = element.attribute("src").await
            && !src.is_empty()
        {
            return decode_image_src(&src);
        }

        if Instant::now() >= deadline {
            bail!("Timed out waiting for login image ({:?})", WAIT_TIMEOUT);
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

fn decode_image_src(src: &str) -> Result<Vec<u8>> {
    let encoded = src
        .strip_prefix("data:")
        .ok_or_else(|| anyhow!("Expected data URL, got: {}..", &src[..src.len().min(30)]))?;
    let base64_data = encoded
        .find(';')
        .and_then(|i| encoded.get(i + 1..))
        .and_then(|s| s.strip_prefix("base64,"))
        .ok_or_else(|| anyhow!("Invalid data URL format"))?;
    base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| anyhow!("Base64 decode error: {e}"))
}

fn save_and_open(data: &[u8]) -> Result<()> {
    let path = env::temp_dir().join(IMAGE_FILENAME);
    fs::write(&path, data).map_err(|e| anyhow!("Failed to save login image: {e}"))?;
    info!("Login image saved to {}", path.display());

    let open_result = if cfg!(target_os = "macos") {
        Command::new("open").arg(&path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "start", "", &path.to_string_lossy()])
            .status()
    } else {
        Command::new("xdg-open").arg(&path).status()
    };

    if let Err(e) = open_result {
        info!("Could not auto-open: {e}. Please open the file manually.");
    }

    Ok(())
}
