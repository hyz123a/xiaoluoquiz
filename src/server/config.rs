use std::{env, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    pub static_dir: PathBuf,
    pub max_connections: u32,
    pub initial_password: String,
    pub initial_admin_username: String,
    pub initial_admin_display_name: String,
    pub session_ttl_seconds: i64,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("DATABASE_URL is required")]
    MissingDatabaseUrl,
    #[error("APP_PORT must be a valid TCP port")]
    InvalidPort(#[source] std::num::ParseIntError),
    #[error("DB_POOL_SIZE must be a positive integer")]
    InvalidPoolSize,
    #[error("DB_POOL_SIZE must be a valid integer")]
    InvalidPoolSizeValue(#[source] std::num::ParseIntError),
    #[error("INITIAL_PASSWORD is required")]
    MissingInitialPassword,
    #[error("INITIAL_PASSWORD must not be empty")]
    EmptyInitialPassword,
    #[error("SESSION_TTL_SECONDS must be a valid integer")]
    InvalidSessionTtl(#[source] std::num::ParseIntError),
    #[error("SESSION_TTL_SECONDS must be at least 60 seconds")]
    InvalidSessionTtlValue,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let database_url = env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;
        let host = env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
        let port = env::var("APP_PORT")
            .unwrap_or_else(|_| "8000".to_owned())
            .parse()
            .map_err(ConfigError::InvalidPort)?;
        let static_dir = env::var("STATIC_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("dist"));
        let max_connections = env::var("DB_POOL_SIZE")
            .unwrap_or_else(|_| "10".to_owned())
            .parse()
            .map_err(ConfigError::InvalidPoolSizeValue)?;

        if max_connections == 0 {
            return Err(ConfigError::InvalidPoolSize);
        }

        let initial_password =
            env::var("INITIAL_PASSWORD").map_err(|_| ConfigError::MissingInitialPassword)?;
        if initial_password.trim().is_empty() {
            return Err(ConfigError::EmptyInitialPassword);
        }
        let initial_admin_username =
            env::var("INITIAL_ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_owned());
        let initial_admin_display_name =
            env::var("INITIAL_ADMIN_DISPLAY_NAME").unwrap_or_else(|_| "系统管理员".to_owned());
        let session_ttl_seconds = env::var("SESSION_TTL_SECONDS")
            .unwrap_or_else(|_| (12 * 60 * 60).to_string())
            .parse()
            .map_err(ConfigError::InvalidSessionTtl)?;
        if session_ttl_seconds < 60 {
            return Err(ConfigError::InvalidSessionTtlValue);
        }

        Ok(Self {
            database_url,
            host,
            port,
            static_dir,
            max_connections,
            initial_password,
            initial_admin_username,
            initial_admin_display_name,
            session_ttl_seconds,
        })
    }
}
