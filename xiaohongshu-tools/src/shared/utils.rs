use crate::t;
use anyhow::{Result, anyhow, bail};
use base64::Engine;
use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams as NetworkEnableParams, EventResponseReceived, GetResponseBodyParams,
};
use chromiumoxide::listeners::EventStream;
use chromiumoxide::page::Page;
use futures::StreamExt;
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

pub async fn wait_for_page_close(page: &Page, interval: Duration) {
    let mut interval = tokio::time::interval(interval);
    loop {
        interval.tick().await;
        match page.evaluate_expression("!!document.documentElement").await {
            Ok(result) => {
                let alive: bool = result.into_value().unwrap_or(false);
                if !alive {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}

pub struct ApiResponseWatcher {
    events: EventStream<EventResponseReceived>,
    url_contains: String,
}

impl ApiResponseWatcher {
    pub async fn start(page: &Page, url_contains: &str) -> Result<Self> {
        page.execute(NetworkEnableParams::default()).await?;
        let events = page.event_listener::<EventResponseReceived>().await?;
        Ok(Self {
            events,
            url_contains: url_contains.to_string(),
        })
    }

    pub async fn wait(self, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut events = self.events;
        let url_contains = self.url_contains;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!(
                    "{}",
                    t!(
                        "utils.api_response_timeout",
                        url = url_contains,
                        timeout = format!("{:?}", timeout)
                    )
                );
            }

            match tokio::time::timeout(remaining, events.next()).await {
                Ok(Some(event)) => {
                    if event.response.url.contains(&url_contains) {
                        return Ok(());
                    }
                }
                Ok(None) | Err(_) => {
                    bail!(
                        "{}",
                        t!(
                            "utils.api_response_timeout",
                            url = url_contains,
                            timeout = format!("{:?}", timeout)
                        )
                    );
                }
            }
        }
    }

    /// Wait for a matching response and return its parsed JSON body.
    /// Uses CDP `Network.getResponseBody` to fetch the response content.
    pub async fn wait_for_body(self, page: &Page, timeout: Duration) -> Result<serde_json::Value> {
        let deadline = Instant::now() + timeout;
        let mut events = self.events;
        let url_contains = self.url_contains;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!(
                    "{}",
                    t!(
                        "utils.api_response_timeout",
                        url = url_contains,
                        timeout = format!("{:?}", timeout)
                    )
                );
            }

            match tokio::time::timeout(remaining, events.next()).await {
                Ok(Some(event)) => {
                    if !event.response.url.contains(&url_contains) {
                        continue;
                    }

                    let params = GetResponseBodyParams::builder()
                        .request_id(event.request_id.clone())
                        .build()
                        .map_err(|e| anyhow!("build GetResponseBodyParams: {e}"))?;

                    let result = page.execute(params).await?;
                    let body: serde_json::Value =
                        serde_json::from_str(&result.body).unwrap_or_default();
                    return Ok(body);
                }
                Ok(None) | Err(_) => {
                    bail!(
                        "{}",
                        t!(
                            "utils.api_response_timeout",
                            url = url_contains,
                            timeout = format!("{:?}", timeout)
                        )
                    );
                }
            }
        }
    }
}
