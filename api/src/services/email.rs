//! Transactional email abstraction. One production implementation
//! ([`ResendEmailSender`]) and one no-op implementation
//! ([`NoopEmailSender`]) used whenever `RESEND_API_KEY` is unset — local
//! dev and every test run this way with no real Resend account needed.
//!
//! Mirrors `services/reverse_geocoder.rs`'s trait + real-impl + stub shape.
//! Unlike the geocoder (which collapses failures to `None` because a missing
//! region label is harmless), email-send failures return a `Result` so the
//! caller can log *why* — every caller is expected to still treat a failure
//! as fail-soft and never let it change the response sent to the end user.

use async_trait::async_trait;

#[async_trait]
pub trait EmailSender: Send + Sync {
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<(), EmailSendError>;
}

/// Type alias for the trait object kept in `AppState`.
pub type SharedEmailSender = std::sync::Arc<dyn EmailSender>;

#[derive(Debug, thiserror::Error)]
pub enum EmailSendError {
    #[error("resend api error (status {status}): {body}")]
    Provider { status: u16, body: String },
    #[error("request failed: {0}")]
    Transport(#[from] reqwest::Error),
}

const RESEND_API_URL: &str = "https://api.resend.com/emails";

/// Sends via Resend's REST API. Resend has no official Rust SDK; the API is
/// a single JSON POST, not worth pulling in a dependency for.
#[derive(Clone)]
pub struct ResendEmailSender {
    client: reqwest::Client,
    api_key: String,
    from_address: String,
}

impl ResendEmailSender {
    pub fn new(api_key: String, from_address: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            from_address,
        }
    }
}

#[async_trait]
impl EmailSender for ResendEmailSender {
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<(), EmailSendError> {
        let resp = self
            .client
            .post(RESEND_API_URL)
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "from": self.from_address,
                "to": [to],
                "subject": subject,
                "html": html_body,
            }))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(EmailSendError::Provider { status, body })
        }
    }
}

/// Test / unconfigured-deployment stub. Logs the attempted send at `info`
/// and always succeeds — no network call, no `RESEND_API_KEY` required.
pub struct NoopEmailSender;

#[async_trait]
impl EmailSender for NoopEmailSender {
    async fn send(&self, to: &str, subject: &str, _html_body: &str) -> Result<(), EmailSendError> {
        tracing::info!(
            to,
            subject,
            "NoopEmailSender: email send skipped (no RESEND_API_KEY configured)"
        );
        Ok(())
    }
}

/// Test double that records every send attempt instead of no-opping
/// silently, so tests can assert on recipient/subject/body. Not
/// `#[cfg(test)]`-gated — integration tests under `api/tests/` link this
/// crate as a normal dependency and need the type to exist in non-test
/// builds too (same convention as `StaticReverseGeocoder`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentEmail {
    pub to: String,
    pub subject: String,
    pub html_body: String,
}

#[derive(Default)]
pub struct RecordingEmailSender {
    pub sent: std::sync::Mutex<Vec<SentEmail>>,
}

#[async_trait]
impl EmailSender for RecordingEmailSender {
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<(), EmailSendError> {
        self.sent.lock().unwrap().push(SentEmail {
            to: to.to_string(),
            subject: subject.to_string(),
            html_body: html_body.to_string(),
        });
        Ok(())
    }
}
