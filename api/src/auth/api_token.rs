//! Org API tokens: machine-to-machine `Authorization: Bearer` credentials,
//! distinct from dashboard sessions (cookie, human) and AppUser sessions
//! (bearer, mobile). See `openspec/specs/org-api-tokens/spec.md`.
//!
//! Every generated token carries the [`TOKEN_PREFIX`] so the auth path can
//! tell it apart from an AppUser session token without a database round
//! trip, and so a leaked value is recognizable in logs/support tickets.
//! Tokens never expire on their own — lifecycle is fully admin-driven
//! (rotate / disable / enable / delete), so unlike session tokens
//! (short-lived, stored as plaintext elsewhere in this codebase) the digest
//! stored at rest is a SHA-256 hash, not the plaintext: a Mongo dump alone
//! must not hand out a permanently-valid credential.

use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use bson::oid::ObjectId;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::domain::ApiTokenScope;
use crate::error::ApiError;
use crate::state::AppState;

pub const TOKEN_PREFIX: &str = "bandao_at_";

const RAW_BYTES: usize = 32;
/// Characters of the random part shown in `token_prefix`, in addition to
/// `TOKEN_PREFIX` itself — enough to visually distinguish tokens in a list,
/// not enough to meaningfully narrow a brute-force search.
const PREFIX_DISPLAY_CHARS: usize = 8;

/// Generates a fresh token secret. Returns `(plaintext, hash, display_prefix)`:
/// `plaintext` is returned to the caller exactly once (creation / rotation)
/// and is never persisted; `hash` is what gets stored for auth lookups;
/// `display_prefix` is a short, non-reconstructable prefix kept for UI
/// recognizability.
pub fn generate() -> (String, String, String) {
    let mut buf = [0u8; RAW_BYTES];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    let random_part = URL_SAFE_NO_PAD.encode(buf);
    let plaintext = format!("{TOKEN_PREFIX}{random_part}");
    let hash = hash_token(&plaintext);
    let shown = &random_part[..PREFIX_DISPLAY_CHARS.min(random_part.len())];
    let display_prefix = format!("{TOKEN_PREFIX}{shown}");
    (plaintext, hash, display_prefix)
}

/// SHA-256 of the plaintext, base64-encoded. Deterministic — same input
/// always hashes to the same output, which is what makes hash-based lookup
/// possible (unlike argon2's per-call random salt).
pub fn hash_token(plaintext: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(plaintext.as_bytes());
    STANDARD.encode(hasher.finalize())
}

/// Per-request context populated by [`api_token_require_session`] once a
/// presented token resolves to an active `org_api_tokens` row.
#[derive(Debug, Clone)]
pub struct ApiTokenAuthContext {
    pub org_id: ObjectId,
    pub scopes: Vec<ApiTokenScope>,
}

impl ApiTokenAuthContext {
    /// Endpoints declare the scope they need and check it explicitly — scope
    /// requirements vary per endpoint, so this isn't baked into a single
    /// static extractor the way `RequireAdmin` bakes in the admin role.
    pub fn require_scope(&self, scope: ApiTokenScope) -> Result<(), ApiError> {
        if self.scopes.contains(&scope) {
            Ok(())
        } else {
            Err(ApiError::Forbidden)
        }
    }
}

impl<S> FromRequestParts<S> for ApiTokenAuthContext
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ApiTokenAuthContext>()
            .cloned()
            .ok_or(ApiError::Unauthorized)
    }
}

#[derive(Debug)]
pub enum ApiTokenAuthFail {
    /// No `Authorization` header, wrong scheme, or the bearer value doesn't
    /// carry `TOKEN_PREFIX` at all — not an API token, nothing to resolve.
    NotPresented,
    /// Carried the prefix but didn't resolve to an active token (unknown
    /// hash, or `status == disabled`). Deliberately not distinguished —
    /// see the `org-api-tokens` spec's "generic unauthorized error" scenario.
    Invalid,
}

/// Core resolution logic, factored out of the axum middleware below so it
/// can be exercised directly against a real `Db` in tests without spinning
/// up an HTTP server or wiring this onto the real router (there is no
/// consumer route yet — see `add-org-api-tokens` tasks 3.4 / 5.5).
pub async fn resolve_from_bearer(
    db: &crate::db::Db,
    bearer_value: &str,
) -> Result<ApiTokenAuthContext, ApiTokenAuthFail> {
    if !bearer_value.starts_with(TOKEN_PREFIX) {
        return Err(ApiTokenAuthFail::NotPresented);
    }
    let hash = hash_token(bearer_value);
    match db.org_api_tokens.find_active_by_hash(&hash).await {
        Ok(Some(token)) => {
            let ctx = ApiTokenAuthContext {
                org_id: token.org_id,
                scopes: token.scopes.clone(),
            };
            if let Err(err) = db.org_api_tokens.touch_last_used(token.id).await {
                tracing::warn!(?err, token_id = %token.id, "failed to update api token last_used_at");
            }
            Ok(ctx)
        }
        Ok(None) => Err(ApiTokenAuthFail::Invalid),
        Err(err) => {
            tracing::error!(?err, "failed to load api token");
            Err(ApiTokenAuthFail::Invalid)
        }
    }
}

/// Middleware for router groups gated on `ApiTokenAuthContext`. Not wired
/// onto any route yet — the first consumer is `add-zhengdan-checkin-export`.
pub async fn api_token_require_session(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    let bearer = match extract_bearer(&headers) {
        Some(t) => t,
        None => return ApiError::Unauthorized.into_response(),
    };
    match resolve_from_bearer(&state.db, &bearer).await {
        Ok(ctx) => {
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
        Err(ApiTokenAuthFail::NotPresented | ApiTokenAuthFail::Invalid) => {
            ApiError::Unauthorized.into_response()
        }
    }
}

fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get(AUTHORIZATION)?.to_str().ok()?;
    let mut parts = value.splitn(2, ' ');
    let scheme = parts.next()?;
    let token = parts.next()?.trim();
    if !scheme.eq_ignore_ascii_case("Bearer") || token.is_empty() {
        return None;
    }
    Some(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_unique_and_carry_the_prefix() {
        let (a_plain, a_hash, a_prefix) = generate();
        let (b_plain, b_hash, b_prefix) = generate();

        assert_ne!(a_plain, b_plain);
        assert_ne!(a_hash, b_hash);
        assert_ne!(a_prefix, b_prefix);
        assert!(a_plain.starts_with(TOKEN_PREFIX));
        assert!(b_plain.starts_with(TOKEN_PREFIX));
        assert!(a_prefix.starts_with(TOKEN_PREFIX));
    }

    #[test]
    fn hash_is_deterministic_for_the_same_input() {
        let (plain, hash, _) = generate();
        assert_eq!(hash_token(&plain), hash);
    }

    #[test]
    fn prefix_never_contains_enough_to_reconstruct_the_token() {
        let (plain, _, prefix) = generate();
        // The display prefix is strictly shorter than the full token, and
        // is not itself a valid token (it wouldn't hash to the stored value).
        assert!(prefix.len() < plain.len());
        assert_ne!(hash_token(&prefix), hash_token(&plain));
    }

    #[test]
    fn extract_bearer_parses_scheme_and_trims() {
        use axum::http::HeaderMap;
        use axum::http::HeaderValue;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer   abc123  "));
        assert_eq!(extract_bearer(&headers).as_deref(), Some("abc123"));

        let mut wrong_scheme = HeaderMap::new();
        wrong_scheme.insert(AUTHORIZATION, HeaderValue::from_static("Basic abc123"));
        assert_eq!(extract_bearer(&wrong_scheme), None);

        assert_eq!(extract_bearer(&HeaderMap::new()), None);
    }
}
