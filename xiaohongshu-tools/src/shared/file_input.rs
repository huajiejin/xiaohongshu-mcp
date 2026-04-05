use crate::shared::utils::poll_until;
use crate::t;
use anyhow::{Result, anyhow};
use chromiumoxide::page::Page;
use std::time::Duration;

#[derive(Debug)]
pub struct FileInputs {
    pub videos: Vec<chromiumoxide::element::Element>,
    pub images: Vec<chromiumoxide::element::Element>,
    pub others: Vec<chromiumoxide::element::Element>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileInputKind {
    Video,
    Image,
    Other,
}

const VIDEO_EXTENSIONS: &[&str] = &[".mp4", ".mov", ".avi", ".webm", ".mkv", ".flv", ".wmv"];
const VIDEO_MIME: &[&str] = &["video/"];
const IMAGE_EXTENSIONS: &[&str] = &[
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".svg", ".tiff",
];
const IMAGE_MIME: &[&str] = &["image/"];

pub fn classify_accept(accept: &str) -> FileInputKind {
    let lower = accept.to_lowercase();

    for ext in VIDEO_EXTENSIONS {
        if lower.contains(ext) {
            return FileInputKind::Video;
        }
    }
    for mime in VIDEO_MIME {
        if lower.contains(mime) {
            return FileInputKind::Video;
        }
    }
    for ext in IMAGE_EXTENSIONS {
        if lower.contains(ext) {
            return FileInputKind::Image;
        }
    }
    for mime in IMAGE_MIME {
        if lower.contains(mime) {
            return FileInputKind::Image;
        }
    }

    FileInputKind::Other
}

pub async fn find_file_inputs(page: &Page) -> Result<FileInputs> {
    let elements = page
        .find_elements(r#"input[type="file"]"#)
        .await
        .map_err(|e| anyhow!("no file inputs found: {e}"))?;

    let mut inputs = FileInputs {
        videos: Vec::new(),
        images: Vec::new(),
        others: Vec::new(),
    };

    for el in elements {
        let accept = el
            .attribute("accept")
            .await
            .ok()
            .flatten()
            .unwrap_or_default();

        match classify_accept(&accept) {
            FileInputKind::Video => inputs.videos.push(el),
            FileInputKind::Image => inputs.images.push(el),
            FileInputKind::Other => inputs.others.push(el),
        }
    }

    Ok(inputs)
}

pub async fn wait_for_file_inputs(
    page: &Page,
    timeout_secs: u64,
    check: impl Fn(&FileInputs) -> bool,
) -> Result<FileInputs> {
    poll_until(
        Duration::from_secs(timeout_secs),
        Duration::from_millis(500),
        || async {
            let inputs = find_file_inputs(page).await.ok()?;
            check(&inputs).then_some(inputs)
        },
    )
    .await
    .map_err(|e| {
        anyhow!(
            "{}",
            t!("file_input.file_input_wait_timeout", error = e.to_string())
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_mp4() {
        assert_eq!(classify_accept(".mp4"), FileInputKind::Video);
    }

    #[test]
    fn test_classify_video_wildcard() {
        assert_eq!(classify_accept("video/*"), FileInputKind::Video);
    }

    #[test]
    fn test_classify_video_case_insensitive() {
        assert_eq!(classify_accept(".MP4,.MOV"), FileInputKind::Video);
    }

    #[test]
    fn test_classify_image_wildcard() {
        assert_eq!(classify_accept("image/*"), FileInputKind::Image);
    }

    #[test]
    fn test_classify_jpg_png() {
        assert_eq!(classify_accept(".jpg,.png,.gif"), FileInputKind::Image);
    }

    #[test]
    fn test_classify_image_case_insensitive() {
        assert_eq!(classify_accept(".JPG,.PNG"), FileInputKind::Image);
    }

    #[test]
    fn test_classify_other_pdf() {
        assert_eq!(classify_accept(".pdf,.doc"), FileInputKind::Other);
    }

    #[test]
    fn test_classify_empty() {
        assert_eq!(classify_accept(""), FileInputKind::Other);
    }

    #[test]
    fn test_classify_video_takes_priority_over_image() {
        assert_eq!(classify_accept(".mp4,.jpg"), FileInputKind::Video);
    }

    #[test]
    fn test_classify_mixed_extensions() {
        assert_eq!(classify_accept(".webm"), FileInputKind::Video);
        assert_eq!(classify_accept(".webp"), FileInputKind::Image);
        assert_eq!(classify_accept(".svg"), FileInputKind::Image);
        assert_eq!(classify_accept(".mkv"), FileInputKind::Video);
    }

    #[test]
    fn test_classify_video_mime_prefix() {
        assert_eq!(classify_accept("video/mp4"), FileInputKind::Video);
    }

    #[test]
    fn test_classify_image_mime_prefix() {
        assert_eq!(classify_accept("image/png"), FileInputKind::Image);
    }
}
