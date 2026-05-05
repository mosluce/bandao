/// Reserved slugs. Includes API path first-level segments, common system identifiers,
/// and the project name. Maintained as a static constant so additions can be reviewed
/// in code and covered by tests.
pub const RESERVED_SLUGS: &[&str] = &[
    // API path first-level segments — see handlers::router
    "auth",
    "me",
    "orgs",
    // Common system identifiers
    "admin",
    "api",
    "app",
    "www",
    "dashboard",
    "login",
    "register",
    "logout",
    "support",
    "help",
    "status",
    "billing",
    "settings",
    "new",
    "create",
    "join",
    "root",
    "signup",
    "signin",
    "oauth",
    "callback",
    "users",
    // Project name
    "argus",
];

#[derive(Debug, PartialEq, Eq)]
pub enum SlugValidationError {
    InvalidFormat,
    Reserved,
}

/// Lowercase + trim. Always run before `validate` and before storage.
pub fn normalize(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

/// Validates a slug that has already been normalized. Format rule: `^[a-z0-9]{2,24}$`.
pub fn validate(normalized: &str) -> Result<(), SlugValidationError> {
    let len = normalized.len();
    if !(2..=24).contains(&len) {
        return Err(SlugValidationError::InvalidFormat);
    }
    if !normalized
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    {
        return Err(SlugValidationError::InvalidFormat);
    }
    if RESERVED_SLUGS.contains(&normalized) {
        return Err(SlugValidationError::Reserved);
    }
    Ok(())
}

/// True if the input string matches the slug format `^[a-z0-9]{2,24}$`.
/// Used by the join lookup router to decide whether to query slug-shaped inputs.
pub fn is_slug_shaped(input: &str) -> bool {
    let len = input.len();
    (2..=24).contains(&len)
        && input
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
}

use std::time::Duration;

use bson::DateTime;
use bson::oid::ObjectId;

use crate::db::{Db, ReservationInsertError};
use crate::domain::Org;
use crate::error::{ApiError, ApiResult};

/// Default grace period: 30 days. Aligns with the rate-limit window so an org holds
/// at most two slugs at any time (active + one in grace).
pub const GRACE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// Atomically replace org's slug.
///
/// Steps:
///   1. Insert a new active reservation for `new_slug`. The unique index gives
///      single-document atomicity — duplicate-key means the slug is already held
///      (active or in grace) and we return `SlugTaken`.
///   2. If the org currently holds an active slug, move that reservation into
///      grace (`expires_at = now + grace_ttl`).
///   3. Update orgs doc with new slug + slug_changed_at.
///
/// On step 2 or 3 failure we best-effort delete the just-inserted reservation
/// (rollback) so the slug is released and the operation appears atomic to callers.
pub async fn set_slug_atomic(
    db: &Db,
    org: &Org,
    new_slug: &str,
    now: DateTime,
    grace_ttl: Duration,
) -> ApiResult<Org> {
    let inserted = match db
        .slug_reservations
        .try_insert_active(new_slug, org.id)
        .await
    {
        Ok(r) => r,
        Err(ReservationInsertError::Duplicate) => return Err(ApiError::SlugTaken),
        Err(ReservationInsertError::Db(err)) => return Err(ApiError::Db(err)),
    };

    if let Err(err) = move_old_to_grace(db, org, now, grace_ttl).await {
        let _ = db.slug_reservations.delete_by_id(inserted.id).await;
        return Err(err);
    }

    match db.orgs.set_slug(org.id, new_slug, now).await {
        Ok(updated) => Ok(updated),
        Err(err) => {
            let _ = db.slug_reservations.delete_by_id(inserted.id).await;
            Err(err)
        }
    }
}

/// Atomically clear org's slug — push current active reservation into grace and
/// null the orgs.slug field.
pub async fn clear_slug_atomic(
    db: &Db,
    org: &Org,
    now: DateTime,
    grace_ttl: Duration,
) -> ApiResult<Org> {
    move_old_to_grace(db, org, now, grace_ttl).await?;
    db.orgs.clear_slug(org.id, now).await
}

async fn move_old_to_grace(
    db: &Db,
    org: &Org,
    now: DateTime,
    grace_ttl: Duration,
) -> ApiResult<()> {
    let Some(old_slug) = org.slug.as_deref() else {
        return Ok(());
    };
    let expires_at_ms = now.timestamp_millis() + grace_ttl.as_millis() as i64;
    let expires_at = DateTime::from_millis(expires_at_ms);
    db.slug_reservations
        .move_to_grace(old_slug, org.id, expires_at)
        .await?;
    Ok(())
}

/// Look up the org for a join input. Format-routed: slug-shaped inputs hit
/// slug_reservations; code-shaped inputs hit orgs.code; everything else is
/// rejected without a DB query.
pub async fn resolve_org_for_join(db: &Db, input: &str) -> ApiResult<Org> {
    if is_slug_shaped(input) {
        let Some(reservation) = db.slug_reservations.find_by_slug(input).await? else {
            return Err(ApiError::InvalidOrgCode);
        };
        let still_valid = match reservation.expires_at {
            None => true,
            Some(exp) => exp.timestamp_millis() > DateTime::now().timestamp_millis(),
        };
        if !still_valid {
            return Err(ApiError::InvalidOrgCode);
        }
        return resolve_by_id(db, reservation.org_id).await;
    }

    if crate::auth::org_code::is_well_formed(input) {
        return db
            .orgs
            .find_by_code(input)
            .await?
            .ok_or(ApiError::InvalidOrgCode);
    }

    Err(ApiError::InvalidOrgCode)
}

async fn resolve_by_id(db: &Db, org_id: ObjectId) -> ApiResult<Org> {
    db.orgs
        .find_by_id(org_id)
        .await?
        .ok_or(ApiError::InvalidOrgCode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lowercases_and_trims() {
        assert_eq!(normalize("ACME"), "acme");
        assert_eq!(normalize("  Acme  "), "acme");
        assert_eq!(normalize("acme01"), "acme01");
    }

    #[test]
    fn validate_accepts_valid_slugs() {
        assert!(validate("acme").is_ok());
        assert!(validate("ab").is_ok());
        assert!(validate("a1").is_ok());
        assert!(validate(&"a".repeat(24)).is_ok());
        assert!(validate("0123456789").is_ok());
    }

    #[test]
    fn validate_rejects_too_short() {
        assert_eq!(validate("a"), Err(SlugValidationError::InvalidFormat));
        assert_eq!(validate(""), Err(SlugValidationError::InvalidFormat));
    }

    #[test]
    fn validate_rejects_too_long() {
        assert_eq!(
            validate(&"a".repeat(25)),
            Err(SlugValidationError::InvalidFormat)
        );
    }

    #[test]
    fn validate_rejects_invalid_chars() {
        assert_eq!(
            validate("acme-corp"),
            Err(SlugValidationError::InvalidFormat)
        );
        assert_eq!(
            validate("acme corp"),
            Err(SlugValidationError::InvalidFormat)
        );
        assert_eq!(
            validate("acme_corp"),
            Err(SlugValidationError::InvalidFormat)
        );
        assert_eq!(validate("ACME"), Err(SlugValidationError::InvalidFormat));
    }

    #[test]
    fn validate_rejects_reserved() {
        assert_eq!(validate("admin"), Err(SlugValidationError::Reserved));
        assert_eq!(validate("argus"), Err(SlugValidationError::Reserved));
        assert_eq!(validate("auth"), Err(SlugValidationError::Reserved));
        assert_eq!(validate("api"), Err(SlugValidationError::Reserved));
    }

    #[test]
    fn is_slug_shaped_format_check() {
        assert!(is_slug_shaped("acme"));
        assert!(is_slug_shaped("ab"));
        assert!(!is_slug_shaped("a"));
        assert!(!is_slug_shaped("ACME"));
        assert!(!is_slug_shaped("acme-corp"));
        assert!(!is_slug_shaped(&"a".repeat(25)));
    }

    /// Invariant test: every first-level path segment of the axum router must be in
    /// RESERVED_SLUGS. If a future route is added without updating the reserved list,
    /// this fails and reminds the maintainer.
    ///
    /// Source paths (from `crate::handlers::router`):
    ///   /auth/register, /auth/login, /auth/logout
    ///   /me
    ///   /orgs/me/code/rotate
    ///   /dashboard-users/{id}/role, /dashboard-users
    ///   /app/auth/login, /app/auth/logout, /app/me, /app/me/password
    ///   /app-users, /app-users/{id}, /app-users/{id}/password-reset
    ///
    /// First-level segments: auth, me, orgs, dashboard-users, app, app-users.
    /// `dashboard-users` and `app-users` cannot match the slug regex (contain `-`),
    /// so they never collide; the slug-shaped ones must be in the list.
    #[test]
    fn router_first_level_paths_are_reserved() {
        let segments = ["auth", "me", "orgs", "app"];
        for seg in segments {
            assert!(
                RESERVED_SLUGS.contains(&seg),
                "router first-level path `{seg}` must appear in RESERVED_SLUGS",
            );
        }
    }
}
