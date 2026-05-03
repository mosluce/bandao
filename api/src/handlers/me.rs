use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum_extra::extract::CookieJar;

use crate::auth::extractor::{AuthContext, build_clearing_cookie};
use crate::error::{ApiError, ApiResult};
use crate::handlers::auth::{AuthResponse, OrgDto, UserDto};
use crate::handlers::users::cascade_self_leave;
use crate::state::AppState;

pub async fn me(State(state): State<AppState>, ctx: AuthContext) -> ApiResult<Json<AuthResponse>> {
    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    Ok(Json(AuthResponse {
        user: UserDto {
            id: user.id.to_hex(),
            email: user.email,
            role: user.role,
        },
        org: OrgDto {
            id: org.id.to_hex(),
            name: org.name,
            code: org.code,
            owner_id: org.owner_id.to_hex(),
            slug: org.slug,
            slug_changed_at: org
                .slug_changed_at
                .and_then(|d| d.try_to_rfc3339_string().ok()),
        },
        role: user.role,
    }))
}

pub async fn leave(
    State(state): State<AppState>,
    jar: CookieJar,
    ctx: AuthContext,
) -> ApiResult<impl IntoResponse> {
    let user = state
        .db
        .dashboard_users
        .find_by_id(ctx.user_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let org = state
        .db
        .orgs
        .find_by_id(ctx.org_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    // Owner cannot self-leave; ownership transfer / org delete are separate flows.
    if user.id == org.owner_id {
        return Err(ApiError::OwnerProtected);
    }

    cascade_self_leave(&state, &user).await?;

    let cleared = jar.remove(build_clearing_cookie());
    Ok((StatusCode::NO_CONTENT, cleared))
}
