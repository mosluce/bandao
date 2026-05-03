use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::Cookie;
use bson::oid::ObjectId;

use crate::domain::Role;
use crate::error::ApiError;

pub const SESSION_COOKIE: &str = "argus_session";

/// Build a clearing cookie that matches the path the live cookie was set with.
/// Browsers only clear an existing cookie when name+path+domain match, so the
/// removal cookie must mirror the live one's `Path=/`.
pub fn build_clearing_cookie() -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE, "")).path("/").build()
}

/// Per-request authentication context. `current_org_id` and `role` are both
/// optional to model the zero-Org state (logged in, not currently bound to any
/// Org). Org-scoped handlers should use `RequireActiveOrg` (or `RequireAdmin`,
/// which delegates to it) to enforce that both fields are populated.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: ObjectId,
    pub current_org_id: Option<ObjectId>,
    pub role: Option<Role>,
    pub session_token: String,
}

impl AuthContext {
    /// Convenience: org-scoped handlers usually want a present `org_id`. This
    /// projection collapses the option pair into the typical "active context"
    /// values, returning `NoActiveOrg` when no Org is selected.
    pub fn require_active_org(&self) -> Result<(ObjectId, Role), ApiError> {
        match (self.current_org_id, self.role) {
            (Some(org_id), Some(role)) => Ok((org_id, role)),
            _ => Err(ApiError::NoActiveOrg),
        }
    }
}

/// Extractor wrapping `AuthContext` plus the resolved active org. Used by
/// any org-scoped handler so the `NoActiveOrg` rejection happens before
/// the handler body runs.
#[derive(Debug, Clone)]
pub struct RequireActiveOrg {
    pub ctx: AuthContext,
    pub org_id: ObjectId,
    pub role: Role,
}

#[derive(Debug, Clone)]
pub struct RequireAdmin(pub RequireActiveOrg);

impl<S> FromRequestParts<S> for AuthContext
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or(ApiError::Unauthorized)
    }
}

impl<S> FromRequestParts<S> for RequireActiveOrg
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ctx = AuthContext::from_request_parts(parts, state).await?;
        let (org_id, role) = ctx.require_active_org()?;
        Ok(RequireActiveOrg { ctx, org_id, role })
    }
}

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let active = RequireActiveOrg::from_request_parts(parts, state).await?;
        if !matches!(active.role, Role::Admin) {
            return Err(ApiError::Forbidden);
        }
        Ok(RequireAdmin(active))
    }
}
