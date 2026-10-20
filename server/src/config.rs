use std::{
    env,
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

use crate::default_static_dir;

pub const DEFAULT_PORT: u16 = 2993;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub static_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let port = match env::var("PORT") {
            Ok(value) => parse_port(&value)?,
            Err(env::VarError::NotPresent) => DEFAULT_PORT,
            Err(env::VarError::NotUnicode(_)) => {
                return Err(ConfigError::InvalidUnicode("PORT"));
            }
        };

        let static_dir = match env::var("STATIC_DIR") {
            Ok(value) => {
                if value.trim().is_empty() {
                    return Err(ConfigError::Empty("STATIC_DIR"));
                }
                PathBuf::from(value)
            }
            Err(env::VarError::NotPresent) => default_static_dir(),
            Err(env::VarError::NotUnicode(_)) => {
                return Err(ConfigError::InvalidUnicode("STATIC_DIR"));
            }
        };

        Ok(Self { port, static_dir })
    }
}

fn parse_port(value: &str) -> Result<u16, ConfigError> {
    let port = value
        .trim()
        .parse::<u16>()
        .map_err(|_| ConfigError::InvalidPort(value.to_owned()))?;

    if port == 0 {
        return Err(ConfigError::InvalidPort(value.to_owned()));
    }

    Ok(port)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    InvalidPort(String),
    InvalidUnicode(&'static str),
    Empty(&'static str),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPort(value) => write!(formatter, "PORT must be 1..=65535, got {value:?}"),
            Self::InvalidUnicode(name) => write!(formatter, "{name} is not valid UTF-8"),
            Self::Empty(name) => write!(formatter, "{name} must not be empty"),
        }
    }
}

impl Error for ConfigError {}

pub fn ensure_static_dir(path: &Path) -> bool {
    path.join("index.html").is_file()
}
