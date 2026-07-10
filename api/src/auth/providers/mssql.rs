//! MSSQL-backed external authentication via `tiberius`.
//!
//! The Org supplies a parameterized query template containing `@account` and
//! `@password` placeholders; we translate those to tiberius positional
//! parameters (`@P1` / `@P2`) and bind the caller's credentials — never string
//! interpolation, so injection in the account/password is impossible. A
//! returned row means the credentials matched; we read the configured
//! `key_col` / `display_col` and just-in-time provision a shadow AppUser.

use std::sync::Arc;

use async_trait::async_trait;
use bson::oid::ObjectId;
use tiberius::{AuthMethod, Client, Config as TiberiusConfig, EncryptionLevel, Row};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;

use super::{AppAuthProvider, AuthProviderError};
use crate::config::Config;
use crate::db::app_users::AppUserRepository;
use crate::domain::EncryptMode;
use crate::domain::{AppUser, ExternalAuthConfig};

pub struct MssqlProvider {
    app_users: AppUserRepository,
    org_id: ObjectId,
    config: ExternalAuthConfig,
    server_config: Arc<Config>,
}

impl MssqlProvider {
    pub fn new(
        app_users: AppUserRepository,
        org_id: ObjectId,
        config: ExternalAuthConfig,
        server_config: Arc<Config>,
    ) -> Self {
        Self {
            app_users,
            org_id,
            config,
            server_config,
        }
    }

    /// Run the configured query with the credentials bound as parameters and
    /// resolve the identity columns. Returns `Ok(None)` when no row matched
    /// (bad credentials). Errors are `Unavailable` with a diagnostic suitable
    /// for the admin-facing test-login endpoint.
    pub async fn resolve_identity(
        &self,
        account: &str,
        password: &str,
    ) -> Result<Option<ResolvedIdentity>, AuthProviderError> {
        // Decrypt the connection password. Absent key / bad ciphertext surfaces
        // as unavailable rather than leaking anything.
        let secret = self
            .server_config
            .secret_box()
            .map_err(|_| unavailable("server encryption key is not configured"))?;
        let conn_password = secret
            .decrypt(&self.config.password_encrypted)
            .map_err(|_| unavailable("stored connection password could not be decrypted"))?;

        let mut cfg = TiberiusConfig::new();
        cfg.host(&self.config.host);
        cfg.port(self.config.port);
        cfg.database(&self.config.database);
        cfg.authentication(AuthMethod::sql_server(
            &self.config.username,
            &conn_password,
        ));
        // Transport encryption is per-Org: legacy on-prem MSSQL often can't do
        // TLS at all (needs Off), so we don't force the tiberius default of
        // Required.
        cfg.encryption(match self.config.encrypt {
            EncryptMode::Off => EncryptionLevel::Off,
            EncryptMode::Optional => EncryptionLevel::On,
            EncryptMode::Required => EncryptionLevel::Required,
        });
        // Only trust an otherwise-invalid (e.g. self-signed) cert when the Org
        // opted in. No effect when encryption is Off.
        if self.config.trust_server_certificate {
            cfg.trust_cert();
        }

        let tcp = TcpStream::connect(cfg.get_addr())
            .await
            .map_err(|e| unavailable(format!("cannot connect to database: {e}")))?;
        tcp.set_nodelay(true)
            .map_err(|e| unavailable(format!("connection setup failed: {e}")))?;

        let mut client = Client::connect(cfg, tcp.compat_write())
            .await
            .map_err(|e| unavailable(format!("database handshake failed: {e}")))?;

        // Translate the org's named placeholders to tiberius positional params.
        let query = self
            .config
            .query
            .replace("@account", "@P1")
            .replace("@password", "@P2");

        let stream = client
            .query(query, &[&account, &password])
            .await
            .map_err(|e| unavailable(format!("query failed: {e}")))?;
        let row = stream
            .into_row()
            .await
            .map_err(|e| unavailable(format!("reading result failed: {e}")))?;

        let Some(row) = row else {
            // No matching row → bad credentials.
            return Ok(None);
        };

        let external_key = column_string(&row, &self.config.key_col)
            .map_err(unavailable)?
            .ok_or_else(|| unavailable("key column is null for the matched row"))?;
        // A blank identifier is unusable as a stable key.
        if external_key.trim().is_empty() {
            return Err(unavailable("key column is empty for the matched row"));
        }
        let display_name = column_string(&row, &self.config.display_col)
            .map_err(unavailable)?
            .unwrap_or_else(|| external_key.clone());

        Ok(Some(ResolvedIdentity {
            external_key,
            display_name,
        }))
    }
}

pub struct ResolvedIdentity {
    pub external_key: String,
    pub display_name: String,
}

#[async_trait]
impl AppAuthProvider for MssqlProvider {
    async fn authenticate(
        &self,
        account: &str,
        password: &str,
    ) -> Result<AppUser, AuthProviderError> {
        let identity = self
            .resolve_identity(account, password)
            .await?
            .ok_or(AuthProviderError::InvalidCredentials)?;

        // Just-in-time provision (or refresh) the shadow user that anchors the
        // session and all downstream check-in data.
        self.app_users
            .upsert_shadow(self.org_id, &identity.external_key, &identity.display_name)
            .await
            .map_err(|e| unavailable(format!("failed to provision local user: {e}")))
    }
}

fn unavailable(msg: impl Into<String>) -> AuthProviderError {
    AuthProviderError::Unavailable(msg.into())
}

/// Read a column by name and coerce it to a `String`, tolerating the common
/// SQL types an identifier/name column might use. `Err` means the column is not
/// present in the result set (a config error worth surfacing distinctly);
/// `Ok(None)` means the column exists but is NULL.
fn column_string(row: &Row, col: &str) -> Result<Option<String>, String> {
    // Verify the column exists so "column not found" is distinguishable from
    // a NULL value in diagnostics.
    let exists = row.columns().iter().any(|c| c.name() == col);
    if !exists {
        return Err(format!("column not found in query result: {col}"));
    }
    if let Ok(v) = row.try_get::<&str, _>(col) {
        return Ok(v.map(|s| s.to_string()));
    }
    if let Ok(v) = row.try_get::<i32, _>(col) {
        return Ok(v.map(|n| n.to_string()));
    }
    if let Ok(v) = row.try_get::<i64, _>(col) {
        return Ok(v.map(|n| n.to_string()));
    }
    if let Ok(v) = row.try_get::<i16, _>(col) {
        return Ok(v.map(|n| n.to_string()));
    }
    if let Ok(v) = row.try_get::<u8, _>(col) {
        return Ok(v.map(|n| n.to_string()));
    }
    if let Ok(v) = row.try_get::<f64, _>(col) {
        return Ok(v.map(|n| n.to_string()));
    }
    if let Ok(v) = row.try_get::<bool, _>(col) {
        return Ok(v.map(|b| b.to_string()));
    }
    Err(format!("unsupported column type for: {col}"))
}
