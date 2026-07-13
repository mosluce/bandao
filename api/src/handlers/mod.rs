pub mod app_auth;
pub mod app_checkin;
pub mod app_dto;
pub mod app_users;
pub mod auth;
pub mod checkin;
pub mod checkin_dto;
pub mod checkin_export;
pub mod external_auth;
pub mod join_requests;
pub mod location_tracking;
pub mod me;
pub mod org_api_tokens;
pub mod orgs;
pub mod users;

use axum::Json;
use axum::Router;
use axum::middleware as axum_middleware;
use axum::routing::{delete, get, patch, post};
use serde_json::{Value, json};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::auth::app_extractor::app_require_session;
use crate::auth::middleware::require_session;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let cors = build_cors(&state);

    let public = Router::new()
        .route("/healthz", get(healthz))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/auth/forgot-password", post(auth::forgot_password))
        .route("/auth/reset-password", post(auth::reset_password))
        .route("/app/auth/login", post(app_auth::login));

    let protected = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/me", get(me::me))
        .route("/me/orgs", post(me::create_org))
        .route("/me/memberships", post(me::join_membership))
        .route(
            "/me/join-requests",
            get(join_requests::list_mine).post(join_requests::submit),
        )
        .route("/me/join-requests/{id}", delete(join_requests::cancel))
        .route("/me/current-org", post(me::switch_current_org))
        .route("/me/leave", post(me::leave))
        .route(
            "/orgs/me/slug",
            post(orgs::set_slug).delete(orgs::clear_slug),
        )
        .route("/orgs/me/owner", post(orgs::transfer_owner))
        .route("/orgs/me/settings", patch(checkin::update_settings))
        .route("/orgs/me/external-auth", post(external_auth::configure))
        .route(
            "/orgs/me/external-auth/test-login",
            post(external_auth::test_login),
        )
        .route("/orgs/me/join-requests", get(join_requests::list_for_org))
        .route(
            "/orgs/me/join-requests/{id}/approve",
            post(join_requests::approve),
        )
        .route(
            "/orgs/me/join-requests/{id}/reject",
            post(join_requests::reject),
        )
        .route("/dashboard-users", get(users::list_in_org))
        .route("/dashboard-users/cooldowns", get(users::list_cooldowns))
        .route(
            "/dashboard-users/cooldowns/{email}",
            delete(users::clear_cooldown),
        )
        .route("/dashboard-users/{id}", delete(users::remove))
        .route("/dashboard-users/{id}/role", patch(users::update_role))
        .route("/dashboard-users/{id}/unlock", post(users::unlock))
        // `/app-users/*` lives in dashboard-tenancy world (cookie auth +
        // RequireAdmin). The route handlers themselves enforce admin role
        // and current-Org scoping.
        .route("/app-users", get(app_users::list).post(app_users::create))
        .route("/app-users/{id}", patch(app_users::update))
        .route(
            "/app-users/{id}/password-reset",
            post(app_users::password_reset),
        )
        .route("/app-users/{id}/unlock", post(app_users::unlock))
        // Admin-side checkin board / per-user history / force-checkout. All
        // three guard on RequireAdmin and scope to current_org inside the
        // handler.
        .route("/checkin/users", get(checkin::list_users))
        .route("/checkin/users/{id}/events", get(checkin::list_user_events))
        .route(
            "/checkin/users/{id}/force-checkout",
            post(checkin::force_checkout),
        )
        .route(
            "/checkin/users/{id}/locations",
            get(location_tracking::list_locations),
        )
        .route(
            "/checkin/users/{id}/locations/export",
            get(location_tracking::export_locations),
        )
        // Org API tokens (machine-to-machine credentials) — admin manages
        // them via a logged-in dashboard session, same as any other
        // org-scoped admin surface. The tokens themselves authenticate a
        // *different* class of request (see `auth::api_token`), not these
        // management endpoints.
        .route(
            "/orgs/me/api-tokens",
            get(org_api_tokens::list).post(org_api_tokens::create),
        )
        .route(
            "/orgs/me/api-tokens/{id}/rotate",
            post(org_api_tokens::rotate),
        )
        .route(
            "/orgs/me/api-tokens/{id}",
            patch(org_api_tokens::update_status).delete(org_api_tokens::delete),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    // `/app/*` (mobile-facing) sits under a separate Bearer-token middleware.
    // `POST /app/auth/login` is public and lives in `public` above.
    let app_protected = Router::new()
        .route("/app/auth/logout", post(app_auth::logout))
        .route("/app/me", get(app_auth::me))
        .route("/app/me/password", post(app_auth::change_password))
        .route(
            "/app/checkin/events",
            post(app_checkin::submit_event).get(app_checkin::list_events),
        )
        .route("/app/checkin/status", get(app_checkin::status))
        .route(
            "/app/checkin/locations",
            post(location_tracking::submit_location_pings),
        )
        .route(
            "/app/checkin/me/locations",
            get(location_tracking::list_my_locations),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            app_require_session,
        ));

    // Machine-to-machine surface, gated on an Org API token (see
    // `auth::api_token`) rather than a dashboard session or AppUser bearer
    // token. First (and currently only) consumer: the Zhengdan checkin
    // export. Each handler still checks its own required scope.
    let api_token_protected = Router::new()
        .route(
            "/orgs/me/checkin/events/export",
            get(checkin_export::export),
        )
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            crate::auth::api_token::api_token_require_session,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .merge(app_protected)
        .merge(api_token_protected)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn healthz() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

fn build_cors(state: &AppState) -> CorsLayer {
    use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
    use axum::http::{HeaderValue, Method};

    let layer = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION, ACCEPT])
        .allow_credentials(true);

    if let Some(origin) = &state.config.allowed_origin {
        match HeaderValue::from_str(origin) {
            Ok(v) => layer.allow_origin(v),
            Err(err) => {
                tracing::warn!(
                    ?err,
                    origin,
                    "invalid BANDAO_ALLOWED_ORIGIN; falling back to no CORS"
                );
                layer
            }
        }
    } else {
        layer
    }
}
