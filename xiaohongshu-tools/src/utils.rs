use anyhow::{Result, anyhow, bail};
use base64::Engine;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub async fn poll_until<F, Fut, T>(
    timeout: Duration,
    interval: Duration,
    mut condition: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Option<T>>,
{
    let deadline = Instant::now() + timeout;

    loop {
        if let Some(value) = condition().await {
            return Ok(value);
        }

        if Instant::now() >= deadline {
            bail!("Timed out after {:?}", timeout);
        }

        tokio::time::sleep(interval).await;
    }
}

pub fn decode_data_url(src: &str) -> Result<Vec<u8>> {
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

pub fn open_file(path: &Path) -> Result<()> {
    let result = if cfg!(target_os = "macos") {
        Command::new("open").arg(path).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/c", "start", "", &path.to_string_lossy()])
            .status()
    } else {
        Command::new("xdg-open").arg(path).status()
    };

    result.map_err(|e| anyhow!("Failed to open file: {e}"))?;
    Ok(())
}
