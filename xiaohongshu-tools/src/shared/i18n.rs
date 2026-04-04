use crate::t;

pub fn init(lang: Option<&str>) {
    let locale = lang.map_or_else(detect_system_locale, |l| l.to_string());
    rust_i18n::set_locale(&locale);
}

fn detect_system_locale() -> String {
    let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    if locale.starts_with("zh") {
        "zh-CN".to_string()
    } else {
        "en".to_string()
    }
}

pub fn cli_about() -> String {
    t!("cli.about").to_string()
}
pub fn cli_auth_about() -> String {
    t!("cli.auth_about").to_string()
}
pub fn cli_auth_login_about() -> String {
    t!("cli.auth_login_about").to_string()
}
pub fn cli_auth_logout_about() -> String {
    t!("cli.auth_logout_about").to_string()
}
pub fn cli_auth_status_about() -> String {
    t!("cli.auth_status_about").to_string()
}
pub fn cli_explore_about() -> String {
    t!("cli.explore_about").to_string()
}
pub fn cli_headless_help() -> String {
    t!("cli.headless_help").to_string()
}
pub fn cli_proxy_help() -> String {
    t!("cli.proxy_help").to_string()
}
pub fn cli_format_help() -> String {
    t!("cli.format_help").to_string()
}
pub fn cli_lang_help() -> String {
    t!("cli.lang_help").to_string()
}
pub fn cli_profile_help() -> String {
    t!("cli.profile_help").to_string()
}
pub fn cli_keywords_help() -> String {
    t!("cli.keywords_help").to_string()
}
pub fn cli_exclude_help() -> String {
    t!("cli.exclude_help").to_string()
}
pub fn cli_max_notes_help() -> String {
    t!("cli.max_notes_help").to_string()
}
pub fn cli_scroll_speed_help() -> String {
    t!("cli.scroll_speed_help").to_string()
}
pub fn cli_interact_help() -> String {
    t!("cli.interact_help").to_string()
}
pub fn cli_duration_help() -> String {
    t!("cli.duration_help").to_string()
}
pub fn cli_search_about() -> String {
    t!("cli.search_about").to_string()
}
pub fn cli_query_help() -> String {
    t!("cli.query_help").to_string()
}
pub fn cli_sort_by_help() -> String {
    t!("cli.sort_by_help").to_string()
}
pub fn cli_note_type_help() -> String {
    t!("cli.note_type_help").to_string()
}
pub fn cli_publish_time_help() -> String {
    t!("cli.publish_time_help").to_string()
}
pub fn cli_search_scope_help() -> String {
    t!("cli.search_scope_help").to_string()
}
pub fn cli_location_help() -> String {
    t!("cli.location_help").to_string()
}
pub fn cli_creator_about() -> String {
    t!("cli.creator_about").to_string()
}
pub fn cli_url_help() -> String {
    t!("cli.url_help").to_string()
}
pub fn cli_open_about() -> String {
    t!("cli.open_about").to_string()
}
pub fn cli_open_url_help() -> String {
    t!("cli.open_url_help").to_string()
}
pub fn cli_note_about() -> String {
    t!("cli.note_about").to_string()
}
pub fn cli_note_url_help() -> String {
    t!("cli.note_url_help").to_string()
}
pub fn cli_max_comments_help() -> String {
    t!("cli.max_comments_help").to_string()
}
pub fn cli_max_replies_help() -> String {
    t!("cli.max_replies_help").to_string()
}
pub fn cli_like_about() -> String {
    t!("cli.like_about").to_string()
}
pub fn cli_favorite_about() -> String {
    t!("cli.favorite_about").to_string()
}
pub fn cli_like_url_help() -> String {
    t!("cli.like_url_help").to_string()
}
pub fn cli_undo_help() -> String {
    t!("cli.undo_help").to_string()
}
pub fn cli_comment_about() -> String {
    t!("cli.comment_about").to_string()
}
pub fn cli_reply_about() -> String {
    t!("cli.reply_about").to_string()
}
pub fn cli_comment_text_help() -> String {
    t!("cli.comment_text_help").to_string()
}
pub fn cli_comment_id_help() -> String {
    t!("cli.comment_id_help").to_string()
}
