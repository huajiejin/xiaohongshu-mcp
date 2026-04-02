use crate::t;
use serde::Serialize;
use std::fmt::Display;

#[derive(Debug, Clone, Copy, Default)]
pub enum Format {
    #[default]
    Text,
    Json,
}

impl std::str::FromStr for Format {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            _ => anyhow::bail!("{}", t!("output.unknown_format", format = s)),
        }
    }
}

pub struct Output {
    format: Format,
}

impl Output {
    pub const fn new(format: Format) -> Self {
        Self { format }
    }

    pub fn result<T: Serialize + Display>(&self, value: &T) {
        match self.format {
            Format::Text => println!("{value}"),
            Format::Json => match serde_json::to_string(value) {
                Ok(json) => println!("{json}"),
                Err(e) => eprintln!("{}", t!("output.serialize_error", e = e.to_string())),
            },
        }
    }
}
