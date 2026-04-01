use anyhow::{Result, anyhow, bail};
use base64::Engine;
use chromiumoxide::page::Page;
use std::time::{Duration, Instant};

const QRCODE_SELECTOR: &str = "#app > div:nth-child(1) > div > div.login-container > div.container > div.code-area > div.qrcode.force-light > img.qrcode-img";

const TERMINAL_WIDTH: u32 = 33;
const WAIT_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub async fn extract_and_render(page: &Page) -> Result<()> {
    let bytes = wait_and_extract_qrcode(page).await?;
    render_qrcode_terminal(&bytes)
}

async fn wait_and_extract_qrcode(page: &Page) -> Result<Vec<u8>> {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    let js_get_src = format!("document.querySelector('{QRCODE_SELECTOR}')?.src || ''");

    loop {
        let src: String = page
            .evaluate(js_get_src.as_str())
            .await
            .map_err(|e| anyhow!("Failed to get QR code img src: {e}"))?
            .into_value()
            .map_err(|e| anyhow!("Failed to parse QR code src: {e}"))?;

        if !src.is_empty() {
            return decode_image_src(page, &src).await;
        }

        if Instant::now() >= deadline {
            bail!("Timed out waiting for QR code to load ({:?})", WAIT_TIMEOUT);
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn decode_image_src(page: &Page, src: &str) -> Result<Vec<u8>> {
    if let Some(encoded) = src.strip_prefix("data:") {
        let base64_data = encoded
            .find(';')
            .and_then(|i| encoded.get(i + 1..))
            .and_then(|s| s.strip_prefix("base64,"))
            .ok_or_else(|| anyhow!("Invalid data URL format"))?;
        return base64::engine::general_purpose::STANDARD
            .decode(base64_data)
            .map_err(|e| anyhow!("Base64 decode error: {e}"));
    }

    let js = format!(
        r#"
        (async () => {{
            const resp = await fetch("{src}");
            const buf = await resp.arrayBuffer();
            return Array.from(new Uint8Array(buf));
        }})()
        "#
    );
    let byte_array: Vec<u8> = page
        .evaluate(js)
        .await
        .map_err(|e| anyhow!("Failed to fetch QR image: {e}"))?
        .into_value()
        .map_err(|e| anyhow!("Failed to parse image bytes: {e}"))?;

    Ok(byte_array)
}

fn render_qrcode_terminal(image_bytes: &[u8]) -> Result<()> {
    let img =
        image::load_from_memory(image_bytes).map_err(|e| anyhow!("Failed to decode image: {e}"))?;

    let gray = img.to_luma8();
    let (orig_w, orig_h) = gray.dimensions();

    let module_size = (orig_w as f32 / TERMINAL_WIDTH as f32).ceil() as u32;
    let cols = orig_w.div_ceil(module_size);
    let rows = orig_h.div_ceil(module_size);
    let half_rows = rows.div_ceil(2);

    let fg = "\x1b[38;2;0;0;0m";
    let bg = "\x1b[48;2;255;255;255m";
    let both = format!("{fg}{bg}");
    let reset = "\x1b[0m";

    let border = format!("{bg}{}\x1b[0m", "  ".repeat(cols as usize + 2));
    println!("{border}");

    for hr in 0..half_rows {
        let top_row = hr * 2;
        let bot_row = hr * 2 + 1;

        print!("{bg}  {reset}");

        for c in 0..cols {
            let top = is_module_dark(&gray, c, top_row, module_size, orig_w, orig_h);
            let bot = is_module_dark(&gray, c, bot_row, module_size, orig_w, orig_h);

            let ch = match (top, bot) {
                (true, true) => "█",
                (true, false) => "▀",
                (false, true) => "▄",
                (false, false) => " ",
            };

            print!("{both}{ch}{reset}");
        }

        println!("{bg}  {reset}");
    }

    println!("{border}");

    Ok(())
}

fn is_module_dark(
    img: &image::GrayImage,
    col: u32,
    row: u32,
    module_size: u32,
    orig_w: u32,
    orig_h: u32,
) -> bool {
    if row * module_size >= orig_h {
        return false;
    }

    let x_start = col * module_size;
    let y_start = row * module_size;
    let x_end = (x_start + module_size).min(orig_w);
    let y_end = (y_start + module_size).min(orig_h);

    let mut dark_count = 0u32;
    let mut total = 0u32;

    for y in y_start..y_end {
        for x in x_start..x_end {
            if img.get_pixel(x, y)[0] < 128 {
                dark_count += 1;
            }
            total += 1;
        }
    }

    total > 0 && dark_count * 2 > total
}
