use crate::t;
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
            bail!(
                "{}",
                t!("utils.timeout", duration = format!("{:?}", timeout))
            );
        }

        tokio::time::sleep(interval).await;
    }
}

pub fn decode_data_url(src: &str) -> Result<Vec<u8>> {
    let prefix = &src[..src.len().min(30)];
    let encoded = src
        .strip_prefix("data:")
        .ok_or_else(|| anyhow!("{}", t!("utils.expected_data_url", prefix = prefix)))?;
    let base64_data = encoded
        .find(';')
        .and_then(|i| encoded.get(i + 1..))
        .and_then(|s| s.strip_prefix("base64,"))
        .ok_or_else(|| anyhow!("{}", t!("utils.invalid_data_url")))?;
    base64::engine::general_purpose::STANDARD
        .decode(base64_data)
        .map_err(|e| anyhow!("{}", t!("utils.base64_error", e = e.to_string())))
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

    result.map_err(|e| anyhow!("{}", t!("utils.open_file_failed", e = e.to_string())))?;
    Ok(())
}
