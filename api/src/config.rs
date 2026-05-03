use std::net::SocketAddr;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub mongo_uri: String,
    pub mongo_db: String,
    pub listen_addr: SocketAddr,
    pub session_ttl: Duration,
    pub cookie_domain: Option<String>,
    pub cookie_secure: bool,
    pub allowed_origin: Option<String>,
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
            "ARGUS_MONGO_URI",
            "mongodb://argus:argus@localhost:27017/argus?authSource=admin",
        );
        let mongo_db = env_or_default("ARGUS_MONGO_DB", "argus");
        let listen_addr_raw = env_or_default("ARGUS_LISTEN_ADDR", "127.0.0.1:8080");
        let listen_addr =
            listen_addr_raw
                .parse::<SocketAddr>()
                .map_err(|e| ConfigError::Invalid {
                    var: "ARGUS_LISTEN_ADDR",
                    message: e.to_string(),
                })?;

        let session_ttl_secs = env_or_default("ARGUS_SESSION_TTL_SECONDS", "1209600")
            .parse::<u64>()
            .map_err(|e| ConfigError::Invalid {
                var: "ARGUS_SESSION_TTL_SECONDS",
                message: e.to_string(),
            })?;
        let session_ttl = Duration::from_secs(session_ttl_secs);

        let cookie_domain = std::env::var("ARGUS_COOKIE_DOMAIN").ok().filter(|v| !v.is_empty());
        let cookie_secure = parse_bool_or("ARGUS_COOKIE_SECURE", false)?;
        let allowed_origin = std::env::var("ARGUS_ALLOWED_ORIGIN").ok().filter(|v| !v.is_empty());

        Ok(Self {
            mongo_uri,
            mongo_db,
            listen_addr,
            session_ttl,
            cookie_domain,
            cookie_secure,
            allowed_origin,
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
