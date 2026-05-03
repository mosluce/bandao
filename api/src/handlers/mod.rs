pub mod auth;
pub mod me;
pub mod orgs;
pub mod users;

use axum::Router;
use axum::middleware as axum_middleware;
use axum::routing::{delete, get, patch, post};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::auth::middleware::require_session;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let cors = build_cors(&state);

    let public = Router::new()
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login));

    let protected = Router::new()
        .route("/auth/logout", post(auth::logout))
        .route("/me", get(me::me))
        .route("/me/orgs", post(me::create_org))
        .route("/me/memberships", post(me::join_membership))
        .route("/me/current-org", post(me::switch_current_org))
        .route("/me/leave", post(me::leave))
        .route("/orgs/me/code/rotate", post(orgs::rotate_code))
        .route(
            "/orgs/me/slug",
            post(orgs::set_slug).delete(orgs::clear_slug),
        )
        .route("/orgs/me/owner", post(orgs::transfer_owner))
        .route("/dashboard-users", get(users::list_in_org))
        .route(
            "/dashboard-users/cooldowns",
            get(users::list_cooldowns),
        )
        .route(
            "/dashboard-users/cooldowns/{email}",
            delete(users::clear_cooldown),
        )
        .route("/dashboard-users/{id}", delete(users::remove))
        .route("/dashboard-users/{id}/role", patch(users::update_role))
        .layer(axum_middleware::from_fn_with_state(
            state.clone(),
            require_session,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
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
                tracing::warn!(?err, origin, "invalid ARGUS_ALLOWED_ORIGIN; falling back to no CORS");
                layer
            }
        }
    } else {
        layer
    }
}
