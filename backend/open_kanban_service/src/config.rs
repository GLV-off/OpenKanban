use std::{net::SocketAddr, str::FromStr};

use anyhow::{Context, Result, bail};

const DEV_JWT_SECRET: &str = "development-only-secret-change-me-please-32+";
const DEFAULT_ACCESS_TTL_SECONDS: u64 = 15 * 60;
const DEFAULT_REFRESH_TTL_SECONDS: u64 = 30 * 24 * 60 * 60;
const DEFAULT_LOGIN_MAX_ATTEMPTS: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    pub fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }
}

impl FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            other => bail!("unknown environment `{other}`, expected `development` or `production`"),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub database_url: String,
    pub log_filter: String,
    pub environment: Environment,
    pub jwt_secret: String,
    pub access_token_ttl_seconds: u64,
    pub refresh_token_ttl_seconds: u64,
    pub allowed_origins: Vec<String>,
    pub cookie_secure: bool,
    pub login_max_attempts: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Builds a configuration from a key/value source. Exposed for tests.
    pub fn from_lookup<F>(lookup: F) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
    {
        let bind = lookup("OK_BIND").unwrap_or_else(|| "127.0.0.1:8080".to_owned());
        let bind = bind
            .parse::<SocketAddr>()
            .with_context(|| format!("OK_BIND is not a socket address: `{bind}`"))?;

        let database_url =
            lookup("OK_DATABASE_URL").unwrap_or_else(|| "sqlite://open_kanban.db".to_owned());
        if !database_url.starts_with("sqlite:") {
            bail!("OK_DATABASE_URL must use the sqlite scheme, got `{database_url}`");
        }

        let log_filter =
            lookup("OK_LOG").unwrap_or_else(|| "info,open_kanban_service=debug".to_owned());
        let environment: Environment = lookup("OK_ENV")
            .unwrap_or_else(|| "development".to_owned())
            .parse()?;

        let jwt_secret = match lookup("OK_JWT_SECRET") {
            Some(secret) if !secret.trim().is_empty() => secret,
            _ if environment.is_production() => {
                bail!("OK_JWT_SECRET is required in production")
            }
            _ => DEV_JWT_SECRET.to_owned(),
        };
        if jwt_secret.len() < 32 {
            bail!("OK_JWT_SECRET must be at least 32 characters long");
        }

        let access_token_ttl_seconds =
            env_u64(&lookup, "OK_ACCESS_TTL_SECONDS", DEFAULT_ACCESS_TTL_SECONDS)?;
        let refresh_token_ttl_seconds = env_u64(
            &lookup,
            "OK_REFRESH_TTL_SECONDS",
            DEFAULT_REFRESH_TTL_SECONDS,
        )?;

        if access_token_ttl_seconds == 0 || refresh_token_ttl_seconds == 0 {
            bail!("token TTL values must be greater than zero");
        }
        if refresh_token_ttl_seconds <= access_token_ttl_seconds {
            bail!("OK_REFRESH_TTL_SECONDS must be greater than OK_ACCESS_TTL_SECONDS");
        }

        let allowed_origins = lookup("OK_ALLOWED_ORIGINS")
            .unwrap_or_else(|| "http://localhost:5173,http://127.0.0.1:5173".to_owned())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let cookie_secure = match lookup("OK_COOKIE_SECURE") {
            Some(value) => parse_bool(&value, "OK_COOKIE_SECURE")?,
            None => environment.is_production(),
        };

        let login_max_attempts =
            env_u64(&lookup, "OK_LOGIN_MAX_ATTEMPTS", DEFAULT_LOGIN_MAX_ATTEMPTS)?;
        if login_max_attempts == 0 {
            bail!("OK_LOGIN_MAX_ATTEMPTS must be greater than zero");
        }
        let login_max_attempts = u32::try_from(login_max_attempts)
            .context("OK_LOGIN_MAX_ATTEMPTS does not fit into u32")?;

        Ok(Self {
            bind,
            database_url,
            log_filter,
            environment,
            jwt_secret,
            access_token_ttl_seconds,
            refresh_token_ttl_seconds,
            allowed_origins,
            cookie_secure,
            login_max_attempts,
        })
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Config")
            .field("bind", &self.bind)
            .field("database_url", &self.database_url)
            .field("log_filter", &self.log_filter)
            .field("environment", &self.environment)
            .field("jwt_secret", &"<redacted>")
            .field("access_token_ttl_seconds", &self.access_token_ttl_seconds)
            .field("refresh_token_ttl_seconds", &self.refresh_token_ttl_seconds)
            .field("allowed_origins", &self.allowed_origins)
            .field("cookie_secure", &self.cookie_secure)
            .field("login_max_attempts", &self.login_max_attempts)
            .finish()
    }
}

fn env_u64<F>(lookup: &F, key: &str, default: u64) -> Result<u64>
where
    F: Fn(&str) -> Option<String>,
{
    match lookup(key) {
        Some(raw) => raw
            .trim()
            .parse::<u64>()
            .with_context(|| format!("{key} must be an unsigned integer, got `{raw}`")),
        None => Ok(default),
    }
}

fn parse_bool(raw: &str, key: &str) -> Result<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        other => bail!("{key} must be a boolean, got `{other}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(values: &[(&str, &str)]) -> Result<Config> {
        Config::from_lookup(|key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        })
    }

    #[test]
    fn uses_development_defaults() {
        let config = config(&[]).expect("valid defaults");

        assert_eq!(config.bind.to_string(), "127.0.0.1:8080");
        assert_eq!(config.database_url, "sqlite://open_kanban.db");
        assert_eq!(config.environment, Environment::Development);
        assert!(config.jwt_secret.len() >= 32);
        assert_eq!(config.access_token_ttl_seconds, DEFAULT_ACCESS_TTL_SECONDS);
        assert_eq!(
            config.refresh_token_ttl_seconds,
            DEFAULT_REFRESH_TTL_SECONDS
        );
        assert_eq!(config.login_max_attempts, 10);
        assert_eq!(
            config.allowed_origins,
            vec![
                "http://localhost:5173".to_owned(),
                "http://127.0.0.1:5173".to_owned()
            ]
        );
        assert!(!config.cookie_secure);
    }

    #[test]
    fn reads_overrides() {
        let config = config(&[
            ("OK_BIND", "0.0.0.0:9000"),
            ("OK_DATABASE_URL", "sqlite://data/kanban.db"),
            ("OK_ENV", "production"),
            ("OK_LOG", "warn"),
            (
                "OK_JWT_SECRET",
                "production-secret-with-at-least-32-characters",
            ),
            ("OK_ACCESS_TTL_SECONDS", "300"),
            ("OK_REFRESH_TTL_SECONDS", "3600"),
            (
                "OK_ALLOWED_ORIGINS",
                " https://kanban.example , https://admin.example ",
            ),
            ("OK_COOKIE_SECURE", "true"),
            ("OK_LOGIN_MAX_ATTEMPTS", "3"),
        ])
        .expect("valid overrides");

        assert_eq!(config.bind.to_string(), "0.0.0.0:9000");
        assert_eq!(config.database_url, "sqlite://data/kanban.db");
        assert_eq!(config.environment, Environment::Production);
        assert_eq!(config.log_filter, "warn");
        assert_eq!(config.access_token_ttl_seconds, 300);
        assert_eq!(config.refresh_token_ttl_seconds, 3600);
        assert_eq!(
            config.allowed_origins,
            vec![
                "https://kanban.example".to_owned(),
                "https://admin.example".to_owned()
            ]
        );
        assert!(config.cookie_secure);
        assert_eq!(config.login_max_attempts, 3);
    }

    #[test]
    fn requires_jwt_secret_in_production() {
        let error = config(&[("OK_ENV", "production")]).expect_err("must fail");
        assert!(error.to_string().contains("OK_JWT_SECRET"));
    }

    #[test]
    fn rejects_short_jwt_secret() {
        let error = config(&[("OK_JWT_SECRET", "too-short")]).expect_err("must fail");
        assert!(error.to_string().contains("32"));
    }

    #[test]
    fn rejects_refresh_ttl_not_greater_than_access_ttl() {
        let error = config(&[
            ("OK_ACCESS_TTL_SECONDS", "600"),
            ("OK_REFRESH_TTL_SECONDS", "600"),
        ])
        .expect_err("must fail");
        assert!(error.to_string().contains("OK_REFRESH_TTL_SECONDS"));
    }

    #[test]
    fn rejects_invalid_bind() {
        let error = config(&[("OK_BIND", "not-an-address")]).expect_err("must fail");
        assert!(error.to_string().contains("OK_BIND"));
    }

    #[test]
    fn rejects_non_sqlite_database() {
        let error =
            config(&[("OK_DATABASE_URL", "postgres://localhost/kanban")]).expect_err("must fail");
        assert!(error.to_string().contains("sqlite"));
    }

    #[test]
    fn rejects_unknown_environment() {
        let error = config(&[("OK_ENV", "staging")]).expect_err("must fail");
        assert!(error.to_string().contains("staging"));
    }

    #[test]
    fn debug_does_not_leak_jwt_secret() {
        let config = config(&[(
            "OK_JWT_SECRET",
            "super-secret-value-with-at-least-32-characters",
        )])
        .expect("valid config");

        let debug = format!("{config:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("super-secret-value"));
    }

    #[test]
    fn rejects_invalid_booleans_and_zero_attempts() {
        assert!(config(&[("OK_COOKIE_SECURE", "maybe")]).is_err());
        assert!(config(&[("OK_LOGIN_MAX_ATTEMPTS", "0")]).is_err());
    }
}
