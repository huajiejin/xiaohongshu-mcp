rust_i18n::i18n!("locales", fallback = "en");

pub mod auth;
pub mod browser;
pub mod commands;
pub mod cookies;
pub mod extractor;
pub mod feed_extract;
pub mod human;
pub mod login_image;
pub mod output;
pub mod retry;
pub mod utils;

pub mod i18n {
    use crate::t;

    pub fn init(lang: Option<&str>) {
        let locale = match lang {
            Some(l) => l.to_string(),
            None => detect_system_locale(),
        };
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
    pub fn cli_keywords_help() -> String {
        t!("cli.keywords_help").to_string()
    }
    pub fn cli_exclude_help() -> String {
        t!("cli.exclude_help").to_string()
    }
    pub fn cli_max_posts_help() -> String {
        t!("cli.max_posts_help").to_string()
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
}

pub use rust_i18n::t;
