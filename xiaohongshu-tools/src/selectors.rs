use crate::shared::utils::poll_until;
use anyhow::Result;
use chromiumoxide::element::Element;
use chromiumoxide::page::Page;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Copy)]
pub enum Selector {
    LoginIndicator,
    QrCodeImage,
    ExploreSections,
    NoteCoverLink,
    NoteTitleSpan,
    CloseBtn,
    CommentScrollerPrimary,
    CommentScrollerFallback,
    CommentScrollerLastResort,
    InteractionContainer,
    CommentsContainer,
    NoCommentsText,
    EndContainer,
    ParentComment,
    ShowMore,
    CommentInputTrigger,
    CommentInputField,
    ReplyButton,
    SubmitButton,
    FilterButton,
    FilterPanel,
    LikeButton,
    FavoriteButton,
    FileInput,
    NoteCoverById,
    CommentById,
    FilterOption,
    PublishBtn,
    CreatorTab,
    Popover,
    ImagePreview,
    TitleInput,
    TitleMaxSuffix,
    EditorContent,
    EditorEmpty,
    ContentLengthError,
    TopicItem,
    ScheduleSwitch,
    DatePickerInput,
    VisibilityDropdown,
    VisibilityOption,
    OriginalSwitchCard,
    OriginalSwitch,
    ModalMask,
    CoverDefault,
    CoverConfirmBtn,
    ExplorePagePublishBtn,
    ActionButton,
    UploadContent,
    VideoUploadProgress,
    VideoPreview,
}

impl Selector {
    pub const fn css(self) -> &'static str {
        match self {
            Self::LoginIndicator => ".main-container .user .link-wrapper .channel",
            Self::QrCodeImage => ".qrcode-img",
            Self::ExploreSections => "#exploreFeeds section",
            Self::NoteCoverLink => "a.cover",
            Self::NoteTitleSpan => "div > div > a > span",
            Self::CloseBtn => "body > div.note-detail-mask > div.close-circle",
            Self::CommentScrollerPrimary => {
                "#noteContainer div.interaction-container > div.note-scroller"
            }
            Self::CommentScrollerFallback => "#noteContainer div.interaction-container",
            Self::CommentScrollerLastResort => "#noteContainer",
            Self::InteractionContainer => ".interaction-container",
            Self::CommentsContainer => ".comments-container",
            Self::NoCommentsText => ".no-comments-text",
            Self::EndContainer => ".end-container",
            Self::ParentComment => ".parent-comment",
            Self::ShowMore => ".show-more",
            Self::CommentInputTrigger => "div.input-box div.content-edit span",
            Self::CommentInputField => "div.input-box div.content-edit p.content-input",
            Self::ReplyButton => ".right .interactions .reply",
            Self::SubmitButton => "div.bottom button.submit",
            Self::FilterButton => "div.filter",
            Self::FilterPanel => "div.filter-panel",
            Self::LikeButton => ".interact-container .left .like-lottie",
            Self::FavoriteButton => ".interact-container .left .reds-icon.collect-icon",
            Self::FileInput => r#"input[type="file"]"#,
            Self::NoteCoverById => "a.cover",
            Self::CommentById => ".parent-comment",
            Self::FilterOption => "div.filter-panel div.filters",
            Self::PublishBtn => ".main-container .channel-list-content > li:nth-child(3)",
            Self::CreatorTab => ".header-tabs .creator-tab",
            Self::Popover => "div.d-popover",
            Self::ImagePreview => ".img-preview-area .pr",
            Self::TitleInput => "div.d-input input",
            Self::TitleMaxSuffix => "div.title-container div.max_suffix",
            Self::EditorContent => ".editor-content p",
            Self::EditorEmpty => ".is-editor-empty",
            Self::ContentLengthError => "div.edit-container div.length-error",
            Self::TopicItem => "#creator-editor-topic-container .item",
            Self::ScheduleSwitch => ".post-time-wrapper .d-switch",
            Self::DatePickerInput => ".date-picker-container input",
            Self::VisibilityDropdown => "div.permission-card-wrapper div.d-select-content",
            Self::VisibilityOption => "div.d-options-wrapper div.d-grid-item div.custom-option",
            Self::OriginalSwitchCard => "div.custom-switch-card",
            Self::OriginalSwitch => "div.d-switch",
            Self::ModalMask => "div.d-modal-mask",
            Self::CoverDefault => "div.publish-page-content-cover-content div.cover > div.default",
            Self::CoverConfirmBtn => "#mojito-btn-container button",
            Self::ExplorePagePublishBtn => {
                ".main-container .channel-list-content > li:nth-child(3)"
            }
            Self::ActionButton => ".publish-page-publish-btn > button",
            Self::UploadContent => "div.upload-content",
            Self::VideoUploadProgress => ".video-upload-progress",
            Self::VideoPreview => ".video-preview, .upload-done, .player-wrapper",
        }
    }

    pub fn js_query(self) -> String {
        format!("document.querySelector('{}')", self.css())
    }

    pub fn js_query_all(self) -> String {
        format!("document.querySelectorAll('{}')", self.css())
    }
}

pub const COMMENT_CONTAINER_SELECTORS: &[Selector] = &[
    Selector::CommentScrollerPrimary,
    Selector::CommentScrollerFallback,
    Selector::CommentScrollerLastResort,
];

pub const SCROLL_CONTAINER_JS_SELECTORS: &[&str] = &[".note-scroller", ".interaction-container"];

pub fn note_cover_by_id_selector(note_id: &str) -> String {
    format!("a.cover[href*='/{note_id}']")
}

pub fn comment_by_id_selector(comment_id: &str) -> String {
    format!("#comment-{comment_id}")
}

pub fn filter_option_selector(group: usize, tag: usize) -> String {
    format!("div.filter-panel div.filters:nth-child({group}) div.tags:nth-child({tag})")
}

pub async fn wait_for_element(page: &Page, sel: Selector) -> Result<Element> {
    wait_for_element_timeout(page, sel, DEFAULT_TIMEOUT).await
}

pub async fn wait_for_element_timeout(
    page: &Page,
    sel: Selector,
    timeout: Duration,
) -> Result<Element> {
    let css = sel.css().to_string();
    poll_until(timeout, DEFAULT_INTERVAL, || {
        let css = css.clone();
        async move { page.find_element(&css).await.ok() }
    })
    .await
}

pub async fn wait_for_elements(page: &Page, sel: Selector) -> Result<Vec<Element>> {
    wait_for_elements_timeout(page, sel, DEFAULT_TIMEOUT).await
}

pub async fn wait_for_elements_timeout(
    page: &Page,
    sel: Selector,
    timeout: Duration,
) -> Result<Vec<Element>> {
    let css = sel.css().to_string();
    poll_until(timeout, DEFAULT_INTERVAL, || {
        let css = css.clone();
        async move {
            let els = page.find_elements(&css).await.ok()?;
            if els.is_empty() { None } else { Some(els) }
        }
    })
    .await
}
