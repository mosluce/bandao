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

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: ObjectId,
    pub org_id: ObjectId,
    pub role: Role,
    pub session_token: String,
}

#[derive(Debug, Clone)]
pub struct RequireAdmin(pub AuthContext);

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

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ctx = AuthContext::from_request_parts(parts, state).await?;
        if !matches!(ctx.role, Role::Admin) {
            return Err(ApiError::Forbidden);
        }
        Ok(RequireAdmin(ctx))
    }
}
