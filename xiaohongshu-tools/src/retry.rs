use anyhow::Result;
use rand::Rng;
use std::time::Duration;
use tracing::warn;

pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_jitter: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_jitter: Duration::from_millis(200),
        }
    }
}

impl RetryConfig {
    pub fn new(max_retries: u32, base_delay: Duration, max_jitter: Duration) -> Self {
        Self {
            max_retries,
            base_delay,
            max_jitter,
        }
    }
}

pub async fn retry<F, Fut, T>(config: &RetryConfig, mut operation: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_err = None;

    for attempt in 0..=config.max_retries {
        match operation().await {
            Ok(val) => return Ok(val),
            Err(e) => {
                if attempt >= config.max_retries {
                    last_err = Some(e);
                    break;
                }

                let exp_delay = config.base_delay * 2u32.pow(attempt);
                let jitter = {
                    let mut rng = rand::rng();
                    let ms = rng.random_range(0..config.max_jitter.as_millis() as u64);
                    Duration::from_millis(ms)
                };
                let total = exp_delay + jitter;

                warn!(
                    "Attempt {}/{} failed: {}. Retrying in {:?}",
                    attempt + 1,
                    config.max_retries,
                    e,
                    total
                );
                tokio::time::sleep(total).await;
            }
        }
    }

    Err(last_err.unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_retry_succeeds_on_first_try() {
        let config = RetryConfig::default();
        let result: Result<i32> = retry(&config, || async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_retry_succeeds_after_failures() {
        let config = RetryConfig::new(3, Duration::from_millis(10), Duration::from_millis(5));
        let count = Arc::new(AtomicU32::new(0));
        let count_clone = count.clone();

        let result = retry(&config, move || {
            let c = count_clone.clone();
            async move {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    anyhow::bail!("fail {}", n)
                }
                Ok(n)
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_retry_exhausts_attempts() {
        let config = RetryConfig::new(2, Duration::from_millis(10), Duration::from_millis(5));
        let result: Result<i32> = retry(&config, || async { anyhow::bail!("always fail") }).await;
        assert!(result.is_err());
    }
}
