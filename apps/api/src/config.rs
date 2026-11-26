//! Configuration from environment variables. Every variable is documented in `.env.example`.

use std::{env, net::SocketAddr, time::Duration};

#[derive(Debug, Clone)]
pub struct Config {
    /// `API_BIND`: address the server listens on.
    pub bind: SocketAddr,
    /// `DATABASE_URL`: Postgres connection string.
    pub database_url: String,
    /// `DATABASE_MAX_CONNECTIONS`: size of the connection pool.
    pub database_max_connections: u32,
    /// `DATABASE_ACQUIRE_TIMEOUT_MS`: how long a request waits for a connection.
    pub database_acquire_timeout: Duration,
    /// `API_MIGRATE_ON_START`: run pending migrations when the server starts.
    pub migrate_on_start: bool,
    /// `API_CORS_ORIGINS`: comma-separated browser origins allowed to call the api.
    pub cors_origins: Vec<String>,
    /// `API_LOG_FORMAT`: `text` for local use, `json` for deployed environments.
    pub log_format: LogFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

pub const DEFAULT_BIND: &str = "127.0.0.1:18081";
pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres@127.0.0.1:15481/chitty";
pub const DEFAULT_CORS_ORIGINS: &str = "http://127.0.0.1:13000";

#[derive(Debug, thiserror::Error)]
#[error("invalid configuration: {name}: {reason}")]
pub struct ConfigError {
    pub name: &'static str,
    pub reason: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    /// Builds the configuration from any variable lookup, so tests need not touch the process environment.
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let var =
            |name: &'static str, default: &str| get(name).unwrap_or_else(|| default.to_string());
        let parse = |name: &'static str, default: &str| -> Result<u64, ConfigError> {
            var(name, default).trim().parse().map_err(|e| ConfigError {
                name,
                reason: format!("{e}"),
            })
        };

        let bind = var("API_BIND", DEFAULT_BIND)
            .parse()
            .map_err(|e| ConfigError {
                name: "API_BIND",
                reason: format!("{e}"),
            })?;
        let migrate_on_start = match var("API_MIGRATE_ON_START", "true").trim() {
            "true" | "1" => true,
            "false" | "0" => false,
            other => {
                return Err(ConfigError {
                    name: "API_MIGRATE_ON_START",
                    reason: format!("expected true or false, got {other:?}"),
                });
            }
        };
        let log_format = match var("API_LOG_FORMAT", "text").trim() {
            "text" => LogFormat::Text,
            "json" => LogFormat::Json,
            other => {
                return Err(ConfigError {
                    name: "API_LOG_FORMAT",
                    reason: format!("expected text or json, got {other:?}"),
                });
            }
        };

        Ok(Self {
            bind,
            database_url: var("DATABASE_URL", DEFAULT_DATABASE_URL),
            database_max_connections: parse("DATABASE_MAX_CONNECTIONS", "10")? as u32,
            database_acquire_timeout: Duration::from_millis(parse(
                "DATABASE_ACQUIRE_TIMEOUT_MS",
                "2000",
            )?),
            migrate_on_start,
            cors_origins: var("API_CORS_ORIGINS", DEFAULT_CORS_ORIGINS)
                .split(',')
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty())
                .collect(),
            log_format,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_apply_when_nothing_is_set() {
        let c = Config::from_lookup(|_| None).unwrap();
        assert_eq!(c.bind.to_string(), DEFAULT_BIND);
        assert_eq!(c.cors_origins, vec![DEFAULT_CORS_ORIGINS.to_string()]);
        assert!(c.migrate_on_start);
        assert_eq!(c.log_format, LogFormat::Text);
    }

    #[test]
    fn cors_origins_are_split_and_trimmed() {
        let c = Config::from_lookup(|n| {
            (n == "API_CORS_ORIGINS").then(|| " http://a.test , http://b.test,".into())
        })
        .unwrap();
        assert_eq!(c.cors_origins, vec!["http://a.test", "http://b.test"]);
    }

    #[test]
    fn invalid_values_name_the_variable() {
        let e = Config::from_lookup(|n| (n == "API_BIND").then(|| "nope".into())).unwrap_err();
        assert_eq!(e.name, "API_BIND");
    }
}
