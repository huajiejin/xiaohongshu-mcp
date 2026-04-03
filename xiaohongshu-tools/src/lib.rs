rust_i18n::i18n!("locales", fallback = "en");

pub mod auth;
pub mod browser;
pub mod commands;
pub mod extract;
pub mod shared;

pub use rust_i18n::t;
