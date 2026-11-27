//! Configuration from environment variables. Every variable is documented in `.env.example`.

use std::{env, fmt, net::SocketAddr, time::Duration};

use base64::Engine;

#[derive(Debug, Clone)]
pub struct Config {
    /// `API_ENV`: where the api runs. Defaults to `production`, so a deployment that forgets
    /// to set it fails closed; only `development` may fall back to development-only defaults
    /// for secrets. Local cargo commands get `development` from `.cargo/config.toml`.
    pub env: AppEnv,
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
    /// `API_CLIENT_IP_HEADER`: header holding the client's IP address, set by the proxy in
    /// front of the api; empty to use the connection's peer address.
    pub client_ip_header: Option<String>,
    /// Staff sign-in settings.
    pub staff_auth: StaffAuthConfig,
}

#[derive(Debug, Clone)]
pub struct StaffAuthConfig {
    /// `STAFF_PASSWORD_MIN_LENGTH`: fewest characters a staff password may have.
    pub password_min_length: usize,
    /// `STAFF_SESSION_IDLE_TIMEOUT_MINUTES`: a session unused for this long is refused.
    pub session_idle_timeout: Duration,
    /// `STAFF_SIGNIN_MAX_ATTEMPTS`: failed attempts allowed per account and per IP address
    /// within the window before sign-in answers 429.
    pub sign_in_max_attempts: u32,
    /// `STAFF_SIGNIN_WINDOW_MINUTES`: the window those attempts are counted in.
    pub sign_in_window: Duration,
    /// `STAFF_SIGNIN_CHALLENGE_TTL_SECONDS`: how long the second step of sign-in stays open.
    pub challenge_ttl: Duration,
    /// `STAFF_ENROLMENT_TOKEN_TTL_HOURS`: how long an enrolment token (from `api create-admin`)
    /// stays valid.
    pub enrolment_token_ttl: Duration,
    /// `TOTP_ENCRYPTION_KEY`: encrypts TOTP secrets at rest.
    pub totp_encryption_key: SecretKey,
}

/// A 256-bit key. Its `Debug` output never shows the key.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey(pub [u8; 32]);

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey([redacted])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppEnv {
    Development,
    Test,
    Staging,
    Production,
}

pub const DEFAULT_BIND: &str = "127.0.0.1:18081";
pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres@127.0.0.1:15481/chitty";
pub const DEFAULT_CORS_ORIGINS: &str = "http://127.0.0.1:13000";

/// Used only when `API_ENV=development` and `TOTP_ENCRYPTION_KEY` is unset. It is public (it
/// is in the source), so it protects nothing: any other environment refuses to start without
/// a real key.
pub const DEVELOPMENT_ONLY_TOTP_KEY: [u8; 32] = *b"chitty-dev-only-totp-key-0000000";

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
        let positive = |name: &'static str, default: &str| -> Result<u64, ConfigError> {
            match parse(name, default)? {
                0 => Err(ConfigError {
                    name,
                    reason: "must be greater than 0".into(),
                }),
                n => Ok(n),
            }
        };

        let env = match var("API_ENV", "production").trim() {
            "development" => AppEnv::Development,
            "test" => AppEnv::Test,
            "staging" => AppEnv::Staging,
            "production" => AppEnv::Production,
            other => {
                return Err(ConfigError {
                    name: "API_ENV",
                    reason: format!(
                        "expected development, test, staging or production, got {other:?}"
                    ),
                });
            }
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
        let client_ip_header = Some(var("API_CLIENT_IP_HEADER", "").trim().to_ascii_lowercase())
            .filter(|h| !h.is_empty());
        if let Some(h) = &client_ip_header {
            axum::http::HeaderName::try_from(h.as_str()).map_err(|e| ConfigError {
                name: "API_CLIENT_IP_HEADER",
                reason: format!("{e}"),
            })?;
        }

        let totp_encryption_key = match get("TOTP_ENCRYPTION_KEY").filter(|k| !k.trim().is_empty())
        {
            Some(k) => parse_key(&k).map_err(|reason| ConfigError {
                name: "TOTP_ENCRYPTION_KEY",
                reason,
            })?,
            None if env == AppEnv::Development => SecretKey(DEVELOPMENT_ONLY_TOTP_KEY),
            None => {
                return Err(ConfigError {
                    name: "TOTP_ENCRYPTION_KEY",
                    reason: "required unless API_ENV=development; generate one with `openssl rand -base64 32`".into(),
                });
            }
        };

        let staff_auth = StaffAuthConfig {
            password_min_length: positive("STAFF_PASSWORD_MIN_LENGTH", "12")? as usize,
            session_idle_timeout: Duration::from_secs(
                positive("STAFF_SESSION_IDLE_TIMEOUT_MINUTES", "720")? * 60,
            ),
            sign_in_max_attempts: positive("STAFF_SIGNIN_MAX_ATTEMPTS", "5")? as u32,
            sign_in_window: Duration::from_secs(
                positive("STAFF_SIGNIN_WINDOW_MINUTES", "15")? * 60,
            ),
            challenge_ttl: Duration::from_secs(positive(
                "STAFF_SIGNIN_CHALLENGE_TTL_SECONDS",
                "300",
            )?),
            enrolment_token_ttl: Duration::from_secs(
                positive("STAFF_ENROLMENT_TOKEN_TTL_HOURS", "24")? * 3600,
            ),
            totp_encryption_key,
        };

        Ok(Self {
            env,
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
            client_ip_header,
            staff_auth,
        })
    }

    /// True when TOTP secrets are encrypted with the public development key.
    pub fn uses_development_totp_key(&self) -> bool {
        self.staff_auth.totp_encryption_key.0 == DEVELOPMENT_ONLY_TOTP_KEY
    }
}

fn parse_key(value: &str) -> Result<SecretKey, String> {
    // The reason never includes the value.
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .map_err(|_| "expected base64".to_string())?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|b: Vec<u8>| format!("expected 32 bytes, got {}", b.len()))?;
    Ok(SecretKey(key))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Looks up test values, with `API_ENV=development` unless the test sets it.
    fn cfg(get: impl Fn(&str) -> Option<String>) -> Result<Config, ConfigError> {
        Config::from_lookup(|n| get(n).or_else(|| (n == "API_ENV").then(|| "development".into())))
    }

    #[test]
    fn api_env_defaults_to_production_and_then_requires_a_totp_key() {
        let e = Config::from_lookup(|_| None).unwrap_err();
        assert_eq!(e.name, "TOTP_ENCRYPTION_KEY");
    }

    #[test]
    fn defaults_apply_when_nothing_is_set() {
        let c = cfg(|_| None).unwrap();
        assert_eq!(c.env, AppEnv::Development);
        assert_eq!(c.bind.to_string(), DEFAULT_BIND);
        assert_eq!(c.cors_origins, vec![DEFAULT_CORS_ORIGINS.to_string()]);
        assert!(c.migrate_on_start);
        assert_eq!(c.log_format, LogFormat::Text);
        assert_eq!(c.client_ip_header, None);
        let a = &c.staff_auth;
        assert_eq!(a.password_min_length, 12);
        assert_eq!(a.session_idle_timeout, Duration::from_secs(12 * 3600));
        assert_eq!(a.sign_in_max_attempts, 5);
        assert_eq!(a.sign_in_window, Duration::from_secs(15 * 60));
        assert_eq!(a.challenge_ttl, Duration::from_secs(300));
        assert_eq!(a.enrolment_token_ttl, Duration::from_secs(24 * 3600));
        assert!(c.uses_development_totp_key());
    }

    #[test]
    fn cors_origins_are_split_and_trimmed() {
        let c =
            cfg(|n| (n == "API_CORS_ORIGINS").then(|| " http://a.test , http://b.test,".into()))
                .unwrap();
        assert_eq!(c.cors_origins, vec!["http://a.test", "http://b.test"]);
    }

    #[test]
    fn invalid_values_name_the_variable() {
        let e = cfg(|n| (n == "API_BIND").then(|| "nope".into())).unwrap_err();
        assert_eq!(e.name, "API_BIND");
    }

    #[test]
    fn outside_development_a_totp_key_is_required() {
        for env in ["test", "staging", "production"] {
            let e = cfg(|n| (n == "API_ENV").then(|| env.into())).unwrap_err();
            assert_eq!(e.name, "TOTP_ENCRYPTION_KEY");
        }
        let key = base64::engine::general_purpose::STANDARD.encode([7u8; 32]);
        let c = cfg(|n| match n {
            "API_ENV" => Some("production".into()),
            "TOTP_ENCRYPTION_KEY" => Some(key.clone()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.staff_auth.totp_encryption_key.0, [7u8; 32]);
        assert!(!c.uses_development_totp_key());
    }

    #[test]
    fn a_malformed_key_is_refused_without_echoing_it() {
        let short = base64::engine::general_purpose::STANDARD.encode([1u8; 16]);
        let e = cfg(|n| (n == "TOTP_ENCRYPTION_KEY").then(|| short.clone())).unwrap_err();
        assert_eq!(e.name, "TOTP_ENCRYPTION_KEY");
        assert!(!e.to_string().contains(&short));
        let e = cfg(|n| (n == "TOTP_ENCRYPTION_KEY").then(|| "not base64!".into())).unwrap_err();
        assert!(!e.to_string().contains("not base64!"));
    }

    #[test]
    fn the_key_is_never_printed() {
        let c = cfg(|_| None).unwrap();
        let printed = format!("{c:?}");
        assert!(printed.contains("[redacted]"));
        assert!(!printed.contains("chitty-dev-only"));
        assert!(!printed.contains(&format!("{:?}", DEVELOPMENT_ONLY_TOTP_KEY)));
    }

    #[test]
    fn zero_limits_are_refused() {
        let e = cfg(|n| (n == "STAFF_SIGNIN_MAX_ATTEMPTS").then(|| "0".into())).unwrap_err();
        assert_eq!(e.name, "STAFF_SIGNIN_MAX_ATTEMPTS");
    }
}
