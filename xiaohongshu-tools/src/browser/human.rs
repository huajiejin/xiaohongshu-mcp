use crate::t;
use anyhow::Result;
use chromiumoxide::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, InsertTextParams,
};
use chromiumoxide::element::Element;
use chromiumoxide::page::Page;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::ops::Range;
use std::time::Duration;

const MIN_SCROLL_DELTA: i64 = 400;

#[derive(Debug, Clone)]
pub struct BehaviorConfig {
    pub human_delay: Range<Duration>,
    pub reaction_time: Range<Duration>,
    pub hover_time: Range<Duration>,
    pub read_time: Range<Duration>,
    pub short_read: Range<Duration>,
    pub scroll_wait: Range<Duration>,
    pub post_scroll: Range<Duration>,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            human_delay: ms(300)..ms(700),
            reaction_time: ms(300)..ms(800),
            hover_time: ms(100)..ms(300),
            read_time: ms(500)..ms(1200),
            short_read: ms(600)..ms(1200),
            scroll_wait: ms(100)..ms(200),
            post_scroll: ms(300)..ms(500),
        }
    }
}

const fn ms(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSpeed {
    Slow,
    Normal,
    Fast,
}

impl ScrollSpeed {
    const fn base_ratio(self) -> f64 {
        match self {
            Self::Slow => 0.5,
            Self::Normal => 0.7,
            Self::Fast => 0.9,
        }
    }

    const fn push_count_range(self) -> Range<u32> {
        match self {
            Self::Slow => 3..6,
            Self::Normal => 2..4,
            Self::Fast => 1..3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Key {
    Enter,
    Backspace,
    Space,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
}

impl Key {
    const fn params(self) -> (&'static str, &'static str, u32) {
        match self {
            Self::Enter => ("Enter", "Enter", 13),
            Self::Backspace => ("Backspace", "Backspace", 8),
            Self::Space => (" ", "Space", 32),
            Self::ArrowLeft => ("ArrowLeft", "ArrowLeft", 37),
            Self::ArrowRight => ("ArrowRight", "ArrowRight", 39),
            Self::ArrowUp => ("ArrowUp", "ArrowUp", 38),
            Self::ArrowDown => ("ArrowDown", "ArrowDown", 40),
        }
    }
}

impl std::str::FromStr for ScrollSpeed {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "slow" => Ok(Self::Slow),
            "normal" => Ok(Self::Normal),
            "fast" => Ok(Self::Fast),
            _ => anyhow::bail!("{}", t!("human.invalid_scroll_speed", speed = s)),
        }
    }
}

#[derive(Debug)]
pub struct ScrollResult {
    pub scrolled: bool,
    pub delta: i64,
    pub scroll_top: i64,
}

pub struct HumanBehavior {
    pub config: BehaviorConfig,
    rng: StdRng,
}

impl Default for HumanBehavior {
    fn default() -> Self {
        Self::new()
    }
}

impl HumanBehavior {
    pub fn new() -> Self {
        Self::with_config(BehaviorConfig::default())
    }

    pub fn with_config(config: BehaviorConfig) -> Self {
        Self {
            config,
            rng: StdRng::from_os_rng(),
        }
    }

    fn gaussian_ms(&mut self, range: Range<Duration>) -> u64 {
        let min = range.start.as_millis() as f64;
        let max = range.end.as_millis() as f64;
        let mean = (min + max) / 2.0;
        let std_dev = (max - min) / 6.0;

        let u1: f64 = self.rng.random();
        let u2: f64 = self.rng.random();
        let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos();

        (mean + z0 * std_dev).clamp(min, max) as u64
    }

    pub async fn random_delay(&mut self, range: Range<Duration>) {
        tokio::time::sleep(Duration::from_millis(self.gaussian_ms(range))).await;
    }

    pub async fn scroll_page(&mut self, page: &Page, speed: ScrollSpeed) -> Result<ScrollResult> {
        let prev_top = eval_i64(page, "document.documentElement.scrollTop").await;
        let viewport = eval_i64(page, "window.innerHeight").await.max(400);

        let base_ratio = speed.base_ratio();
        let push_count = self.rng.random_range(speed.push_count_range());

        for _ in 0..push_count {
            let noise_ratio: f64 = self.rng.random_range(0.0..0.2);
            let pixel_noise: i64 = self.rng.random_range(-50..50);
            let delta = ((viewport as f64 * (base_ratio + noise_ratio)) as i64 + pixel_noise)
                .max(MIN_SCROLL_DELTA);

            let js = format!("window.scrollBy({{top:{delta},behavior:'smooth'}})");
            let _ = page.evaluate_expression(&js).await;
            self.random_delay(self.config.scroll_wait.clone()).await;
        }

        let new_top = eval_i64(page, "document.documentElement.scrollTop").await;
        let scrolled = new_top > prev_top;

        if !scrolled {
            let _ = page
                .evaluate_expression("window.scrollTo(0,document.body.scrollHeight)")
                .await;
        }

        self.random_delay(self.config.post_scroll.clone()).await;

        let final_top = eval_i64(page, "document.documentElement.scrollTop").await;

        Ok(ScrollResult {
            scrolled: final_top > prev_top,
            delta: final_top - prev_top,
            scroll_top: final_top,
        })
    }

    pub async fn smart_scroll(&mut self, page: &Page, delta: i64) -> Result<()> {
        let js = format!(
            r#"(() => {{
                const t = document.querySelector('.note-scroller')
                    || document.querySelector('.interaction-container')
                    || document.documentElement;
                t.dispatchEvent(new WheelEvent('wheel',{{
                    deltaY:{delta},deltaMode:0,bubbles:true,cancelable:true,view:window
                }}));
            }})()"#
        );
        page.evaluate_expression(&js).await?;
        Ok(())
    }

    pub async fn human_click(&mut self, page: &Page, element: &Element) -> Result<()> {
        element.scroll_into_view().await?;
        self.random_delay(self.config.reaction_time.clone()).await;

        let point = element.clickable_point().await?;

        if point.x >= 0.0 && point.y >= 0.0 {
            page.move_mouse(point).await?;
            self.random_delay(self.config.hover_time.clone()).await;
            element.click().await?;
        } else {
            element
                .call_js_fn("function() { this.click(); }", false)
                .await?;
        }

        self.random_delay(self.config.read_time.clone()).await;
        Ok(())
    }

    pub async fn popup_click(
        &mut self,
        page: &Page,
        anchor: &Element,
        target: &Element,
    ) -> Result<()> {
        let anchor_point = anchor.clickable_point().await?;
        page.move_mouse(anchor_point).await?;
        self.random_delay(self.config.reaction_time.clone()).await;

        target
            .call_js_fn("function() { this.click(); }", false)
            .await?;
        self.random_delay(self.config.hover_time.clone()).await;

        Ok(())
    }

    /// ASCII-only typing via per-character key dispatch. Use for standard keyboard input.
    /// Panics on CJK/emoji — use `human_type_text` for Unicode content.
    pub async fn human_type(&mut self, page: &Page, element: &Element, text: &str) -> Result<()> {
        element.scroll_into_view().await?;
        self.random_delay(self.config.reaction_time.clone()).await;

        let point = element.clickable_point().await?;
        page.move_mouse(point).await?;
        self.random_delay(self.config.hover_time.clone()).await;

        element.click().await?;
        self.random_delay(self.config.human_delay.clone()).await;

        element.type_str(text).await?;

        self.random_delay(self.config.read_time.clone()).await;
        Ok(())
    }

    /// Unicode-safe text insertion via CDP Input.insertText. Use for CJK, emoji, or mixed content.
    pub async fn human_type_text(
        &mut self,
        page: &Page,
        element: &Element,
        text: &str,
    ) -> Result<()> {
        element.scroll_into_view().await?;
        self.random_delay(self.config.reaction_time.clone()).await;

        let point = element.clickable_point().await?;
        page.move_mouse(point).await?;
        self.random_delay(self.config.hover_time.clone()).await;

        element.click().await?;
        self.random_delay(self.config.human_delay.clone()).await;

        page.execute(InsertTextParams::new(text)).await?;

        self.random_delay(self.config.read_time.clone()).await;
        Ok(())
    }

    pub async fn simulate_reading(&mut self, content_length: usize) {
        let base = self.config.read_time.start.as_millis() as u64;
        let extra = (content_length as u64 / 100).min(2000);
        tokio::time::sleep(Duration::from_millis(base + extra)).await;
    }

    pub async fn clear_and_input(
        &mut self,
        page: &Page,
        element: &Element,
        new_content: Option<&str>,
    ) -> Result<()> {
        self.human_click(page, element).await?;

        let current_len: usize = element
            .call_js_fn(
                "function(){return(this.value||this.innerText||'').length}",
                false,
            )
            .await
            .ok()
            .and_then(|r| r.result.value)
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        page.execute(InsertTextParams::new(" ")).await?;
        self.random_delay(Duration::from_millis(30)..Duration::from_millis(60))
            .await;

        Self::press_key(page, Key::ArrowLeft).await?;
        self.random_delay(Duration::from_millis(30)..Duration::from_millis(60))
            .await;

        let extra_backspaces = current_len + self.rng.random_range(5..10);
        for _ in 0..extra_backspaces {
            Self::press_key(page, Key::Backspace).await?;
            self.random_delay(Duration::from_millis(20)..Duration::from_millis(60))
                .await;
        }

        if let Some(content) = new_content
            && !content.is_empty()
        {
            page.execute(InsertTextParams::new(content)).await?;
            self.random_delay(Duration::from_millis(30)..Duration::from_millis(60))
                .await;
        }

        Self::press_key(page, Key::ArrowRight).await?;
        self.random_delay(Duration::from_millis(30)..Duration::from_millis(60))
            .await;

        let has_trailing_space: bool = element
            .call_js_fn("function(){let v=this.value||this.innerText||'';return v.length>0&&v.endsWith(' ')}", false)
            .await
            .ok()
            .and_then(|r| r.result.value)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if has_trailing_space {
            Self::press_key(page, Key::Backspace).await?;
        }

        Self::press_key(page, Key::Enter).await?;
        self.random_delay(self.config.read_time.clone()).await;
        Ok(())
    }

    pub async fn press_key(page: &Page, key: Key) -> Result<()> {
        let (key_name, code, vk) = key.params();
        let down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key(key_name)
            .code(code)
            .windows_virtual_key_code(vk)
            .build()
            .map_err(|e| anyhow::anyhow!("DispatchKeyEvent build: {e}"))?;
        page.execute(down).await?;

        let up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key(key_name)
            .code(code)
            .windows_virtual_key_code(vk)
            .build()
            .map_err(|e| anyhow::anyhow!("DispatchKeyEvent build: {e}"))?;
        page.execute(up).await?;

        Ok(())
    }

    pub async fn scroll_container(&mut self, page: &Page, selectors: &[&str]) -> Result<()> {
        let max_iterations = 5 + self.rng.random_range(0..6);
        let selector_arr: String = selectors
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(",");

        let mut prev_top = -1i64;

        for i in 0..max_iterations {
            let find_js = format!(
                "(()=>{{const ss=[{selector_arr}];for(const s of ss){{const el=document.querySelector(s);if(el)return{{found:true,scrollTop:el.scrollTop,clientHeight:el.clientHeight}};}}return{{found:false}};}})()"
            );
            let info = page
                .evaluate_expression(&find_js)
                .await?
                .into_value::<serde_json::Value>()?;

            if !info["found"].as_bool().unwrap_or(false) {
                break;
            }

            let scroll_top = info["scrollTop"].as_i64().unwrap_or(0);
            let client_height = info["clientHeight"].as_i64().unwrap_or(400);

            if scroll_top == prev_top && i > 0 {
                break;
            }
            prev_top = scroll_top;

            let amount = (client_height as f64 * 0.7) as i64;
            let scroll_js = format!(
                "(()=>{{const ss=[{selector_arr}];for(const s of ss){{const el=document.querySelector(s);if(el){{el.scrollBy({{top:{amount},behavior:'smooth'}});return true;}}}}return false;}})()"
            );
            let _ = page.evaluate_expression(&scroll_js).await;

            let wait = self.rng.random_range(1500u64..3000);
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }

        Ok(())
    }
}

async fn eval_i64(page: &Page, expr: &str) -> i64 {
    page.evaluate_expression(expr)
        .await
        .ok()
        .and_then(|r| r.into_value::<serde_json::Value>().ok())
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
}
