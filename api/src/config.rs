use std::net::SocketAddr;
use std::time::Duration;

use crate::auth::secret_box::{self, SecretBox};

#[derive(Debug, Clone)]
pub struct Config {
    pub mongo_uri: String,
    pub mongo_db: String,
    pub listen_addr: SocketAddr,
    pub session_ttl: Duration,
    pub cookie_domain: Option<String>,
    pub cookie_secure: bool,
    pub allowed_origin: Option<String>,
    /// AEAD key for encrypting external-auth DB connection passwords, decoded
    /// from `BANDAO_SECRET_KEY` (base64 of 32 bytes). `None` when the env var is
    /// unset — deployments without external auth don't need it.
    pub secret_key: Option<[u8; 32]>,
    /// Resend API key, from `RESEND_API_KEY`. `None` when unset — `AppState`
    /// falls back to `NoopEmailSender` (logs, sends nothing) so no code path
    /// outside production requires a real Resend account.
    pub resend_api_key: Option<String>,
    /// From-address for outbound email, e.g. `"班到 <noreply@ccmos.tw>"`.
    /// From `RESEND_FROM_ADDRESS`; a dev-friendly default so local runs don't
    /// need it set (the address is irrelevant when `resend_api_key` is `None`).
    pub email_from_address: String,
    /// Public base URL of admin-web, used to build links embedded in email
    /// (e.g. the password-reset link). From `ADMIN_WEB_BASE_URL`.
    pub admin_web_base_url: String,
}

impl Config {
    /// A [`SecretBox`] built from the configured key, or
    /// `ExternalAuthUnavailable` when `BANDAO_SECRET_KEY` is not configured —
    /// external auth cannot encrypt/decrypt connection passwords without it.
    pub fn secret_box(&self) -> crate::error::ApiResult<SecretBox> {
        self.secret_key
            .as_ref()
            .map(SecretBox::from_key_bytes)
            .ok_or(crate::error::ApiError::ExternalAuthUnavailable)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing env var: {0}")]
    Missing(&'static str),
    #[error("invalid env var {var}: {message}")]
    Invalid { var: &'static str, message: String },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mongo_uri = env_or_default(
            "BANDAO_MONGO_URI",
            "mongodb://bandao:bandao@localhost:27017/bandao?authSource=admin",
        );
        let mongo_db = env_or_default("BANDAO_MONGO_DB", "bandao");
        let listen_addr_raw = env_or_default("BANDAO_LISTEN_ADDR", "127.0.0.1:8080");
        let listen_addr =
            listen_addr_raw
                .parse::<SocketAddr>()
                .map_err(|e| ConfigError::Invalid {
                    var: "BANDAO_LISTEN_ADDR",
                    message: e.to_string(),
                })?;

        let session_ttl_secs = env_or_default("BANDAO_SESSION_TTL_SECONDS", "1209600")
            .parse::<u64>()
            .map_err(|e| ConfigError::Invalid {
                var: "BANDAO_SESSION_TTL_SECONDS",
                message: e.to_string(),
            })?;
        let session_ttl = Duration::from_secs(session_ttl_secs);

        let cookie_domain = std::env::var("BANDAO_COOKIE_DOMAIN")
            .ok()
            .filter(|v| !v.is_empty());
        let cookie_secure = parse_bool_or("BANDAO_COOKIE_SECURE", false)?;
        let allowed_origin = std::env::var("BANDAO_ALLOWED_ORIGIN")
            .ok()
            .filter(|v| !v.is_empty());

        let secret_key = match std::env::var("BANDAO_SECRET_KEY") {
            Ok(v) if !v.is_empty() => {
                Some(secret_box::decode_key(&v).ok_or(ConfigError::Invalid {
                    var: "BANDAO_SECRET_KEY",
                    message: "expected base64 of exactly 32 bytes".to_string(),
                })?)
            }
            _ => None,
        };

        let resend_api_key = std::env::var("RESEND_API_KEY")
            .ok()
            .filter(|v| !v.is_empty());
        let email_from_address =
            env_or_default("RESEND_FROM_ADDRESS", "班到 <onboarding@resend.dev>");
        let admin_web_base_url = env_or_default("ADMIN_WEB_BASE_URL", "http://localhost:3000");

        Ok(Self {
            mongo_uri,
            mongo_db,
            listen_addr,
            session_ttl,
            cookie_domain,
            cookie_secure,
            allowed_origin,
            secret_key,
            resend_api_key,
            email_from_address,
            admin_web_base_url,
        })
    }
}

fn env_or_default(var: &str, default: &str) -> String {
    std::env::var(var)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_bool_or(var: &'static str, default: bool) -> Result<bool, ConfigError> {
    match std::env::var(var) {
        Ok(v) if v.is_empty() => Ok(default),
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(ConfigError::Invalid {
                var,
                message: format!("expected boolean, got `{v}`"),
            }),
        },
        Err(_) => Ok(default),
    }
}
