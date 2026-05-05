use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::CookieJar;

use crate::auth::extractor::{AuthContext, SESSION_COOKIE, build_clearing_cookie};
use crate::error::ApiError;
use crate::state::AppState;

enum AuthFail {
    /// No / unknown session — return 401 without clobbering the cookie.
    Missing,
    /// Session existed but is no longer usable (expired, identity gone, or
    /// `current_org_id` no longer maps to an active membership). Clear the
    /// cookie so the browser stops re-presenting it.
    Stale,
}

pub async fn require_session(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Response {
    match authenticate(&state, &jar).await {
        Ok(ctx) => {
            // Sliding refresh: extend the session window. Failures here are non-fatal.
            if let Err(err) = state
                .db
                .dashboard_sessions
                .touch_expires(&ctx.session_token, state.config.session_ttl)
                .await
            {
                tracing::warn!(?err, "failed to slide session expiry");
            }
            req.extensions_mut().insert(ctx);
            next.run(req).await
        }
        Err(AuthFail::Missing) => ApiError::Unauthorized.into_response(),
        Err(AuthFail::Stale) => {
            // Clear the cookie on stale-session — must match the original Path=/ to actually overwrite.
            let cleared = jar.remove(build_clearing_cookie());
            (cleared, ApiError::Unauthorized.into_response()).into_response()
        }
    }
}

async fn authenticate(state: &AppState, jar: &CookieJar) -> Result<AuthContext, AuthFail> {
    let token = match jar.get(SESSION_COOKIE) {
        Some(c) => c.value().to_string(),
        None => return Err(AuthFail::Missing),
    };

    let session = match state.db.dashboard_sessions.find_by_token(&token).await {
        Ok(Some(s)) => s,
        Ok(None) => return Err(AuthFail::Missing),
        Err(err) => {
            tracing::error!(?err, "failed to load session");
            return Err(AuthFail::Missing);
        }
    };

    let now_ms = bson::DateTime::now().timestamp_millis();
    if session.expires_at.timestamp_millis() < now_ms {
        let _ = state.db.dashboard_sessions.delete_by_token(&token).await;
        return Err(AuthFail::Stale);
    }

    // Identity must still exist. A missing user with an active session is a
    // hard inconsistency — drop the session and clear the cookie.
    let user = match state.db.dashboard_users.find_by_id(session.user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            let _ = state.db.dashboard_sessions.delete_by_token(&token).await;
            return Err(AuthFail::Stale);
        }
        Err(err) => {
            tracing::error!(?err, "failed to load user");
            return Err(AuthFail::Missing);
        }
    };

    // Resolve role from the membership table (no caching on the session row).
    // If `current_org_id` is set but the membership is gone, the session is
    // stale — force the client to re-login.
    let role = match session.current_org_id {
        Some(org_id) => {
            match state
                .db
                .dashboard_memberships
                .find_by_user_and_org(user.id, org_id)
                .await
            {
                Ok(Some(m)) => Some(m.role),
                Ok(None) => return Err(AuthFail::Stale),
                Err(err) => {
                    tracing::error!(?err, "failed to load membership");
                    return Err(AuthFail::Missing);
                }
            }
        }
        None => None,
    };

    Ok(AuthContext {
        user_id: user.id,
        current_org_id: session.current_org_id,
        role,
        session_token: token,
    })
}
