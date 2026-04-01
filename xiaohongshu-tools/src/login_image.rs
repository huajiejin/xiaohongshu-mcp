use crate::utils;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use std::env;
use std::fs;
use std::time::Duration;
use tracing::info;

const IMAGE_SELECTOR: &str = ".qrcode-img";

const WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const IMAGE_FILENAME: &str = "xhs-login.png";

pub async fn fetch_and_open(page: &Page) -> Result<()> {
    let data = fetch_image_bytes(page).await?;
    save_and_open(&data)
}

async fn fetch_image_bytes(page: &Page) -> Result<Vec<u8>> {
    let p = page.clone();
    utils::poll_until(WAIT_TIMEOUT, POLL_INTERVAL, || {
        let p = p.clone();
        async move {
            let element = p.find_element(IMAGE_SELECTOR).await.ok()?;
            let src = element.attribute("src").await.ok()??;
            if src.is_empty() {
                return None;
            }
            utils::decode_data_url(&src).ok()
        }
    })
    .await
    .map_err(|_| anyhow!("Timed out waiting for login image ({:?})", WAIT_TIMEOUT))
}

fn save_and_open(data: &[u8]) -> Result<()> {
    let path = env::temp_dir().join(IMAGE_FILENAME);
    fs::write(&path, data).map_err(|e| anyhow!("Failed to save login image: {e}"))?;
    info!("Login image saved to {}", path.display());

    if let Err(e) = utils::open_file(&path) {
        info!("Could not auto-open: {e}. Please open the file manually.");
    }

    Ok(())
}
