pub mod app_sessions;
pub mod app_users;
pub mod checkin_events;
pub mod checkin_user_status;
pub mod dashboard_memberships;
pub mod dashboard_sessions;
pub mod dashboard_users;
pub mod join_requests;
pub mod location_pings;
pub mod org_api_tokens;
pub mod orgs;
pub mod password_reset_tokens;
pub mod removed_memberships;
pub mod slug_reservations;

use std::time::Duration;

use bson::doc;
use mongodb::options::{ClientOptions, IndexOptions};
use mongodb::{Client, Collection, Database, IndexModel};

use crate::domain::{
    AppSession, AppUser, CheckinEvent, CheckinUserStatus, DashboardSession, DashboardUser,
    JoinRequest, LocationPing, Membership, Org, OrgApiToken, OrgSlugReservation,
    PasswordResetToken, RemovedMembership,
};
use crate::error::ApiResult;

pub use app_sessions::AppSessionRepository;
pub use app_users::{AppUserInsertError, AppUserRepository};
pub use checkin_events::CheckinEventRepository;
pub use checkin_user_status::{CheckinStatusInsertError, CheckinUserStatusRepository};
pub use dashboard_memberships::{MembershipInsertError, MembershipRepository};
pub use dashboard_sessions::DashboardSessionRepository;
pub use dashboard_users::DashboardUserRepository;
pub use join_requests::{JoinRequestInsertError, JoinRequestRepository};
pub use location_pings::{InsertManyOutcome, LOCATION_PING_BATCH_MAX, LocationPingRepository};
pub use org_api_tokens::OrgApiTokenRepository;
pub use orgs::OrgRepository;
pub use password_reset_tokens::PasswordResetTokenRepository;
pub use removed_memberships::RemovedMembershipRepository;
pub use slug_reservations::{OrgSlugReservationRepository, ReservationInsertError};

#[derive(Clone)]
pub struct Db {
    pub database: Database,
    pub orgs: OrgRepository,
    pub dashboard_users: DashboardUserRepository,
    pub dashboard_memberships: MembershipRepository,
    pub dashboard_sessions: DashboardSessionRepository,
    pub slug_reservations: OrgSlugReservationRepository,
    pub removed_memberships: RemovedMembershipRepository,
    pub app_users: AppUserRepository,
    pub app_sessions: AppSessionRepository,
    pub checkin_events: CheckinEventRepository,
    pub checkin_user_status: CheckinUserStatusRepository,
    pub location_pings: LocationPingRepository,
    pub join_requests: JoinRequestRepository,
    pub org_api_tokens: OrgApiTokenRepository,
    pub password_reset_tokens: PasswordResetTokenRepository,
}

impl Db {
    pub async fn connect(uri: &str, db_name: &str) -> ApiResult<Self> {
        let mut options = ClientOptions::parse(uri).await?;
        options.app_name = Some("bandao-api".to_string());
        let client = Client::with_options(options)?;

        // Eager ping so misconfiguration fails at boot, not on first request.
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await?;

        let database = client.database(db_name);
        Ok(Self {
            orgs: OrgRepository::new(database.collection::<Org>("orgs")),
            dashboard_users: DashboardUserRepository::new(
                database.collection::<DashboardUser>("dashboard_users"),
            ),
            dashboard_memberships: MembershipRepository::new(
                database.collection::<Membership>("dashboard_memberships"),
            ),
            dashboard_sessions: DashboardSessionRepository::new(
                database.collection::<DashboardSession>("dashboard_sessions"),
            ),
            slug_reservations: OrgSlugReservationRepository::new(
                database.collection::<OrgSlugReservation>("slug_reservations"),
            ),
            removed_memberships: RemovedMembershipRepository::new(
                database.collection::<RemovedMembership>("removed_memberships"),
            ),
            app_users: AppUserRepository::new(database.collection::<AppUser>("app_users")),
            app_sessions: AppSessionRepository::new(
                database.collection::<AppSession>("app_sessions"),
            ),
            checkin_events: CheckinEventRepository::new(
                database.collection::<CheckinEvent>("checkin_events"),
            ),
            checkin_user_status: CheckinUserStatusRepository::new(
                database.collection::<CheckinUserStatus>("checkin_user_status"),
            ),
            location_pings: LocationPingRepository::new(
                database.collection::<LocationPing>("location_pings"),
            ),
            join_requests: JoinRequestRepository::new(
                database.collection::<JoinRequest>("join_requests"),
            ),
            org_api_tokens: OrgApiTokenRepository::new(
                database.collection::<OrgApiToken>("org_api_tokens"),
            ),
            password_reset_tokens: PasswordResetTokenRepository::new(
                database.collection::<PasswordResetToken>("password_reset_tokens"),
            ),
            database,
        })
    }

    pub async fn ensure_indexes(&self) -> ApiResult<()> {
        let orgs: Collection<Org> = self.database.collection("orgs");
        orgs.create_index(
            IndexModel::builder()
                .keys(doc! { "code": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .name("orgs_code_unique".to_string())
                        .build(),
                )
                .build(),
        )
        .await?;
        orgs.create_index(
            IndexModel::builder()
                .keys(doc! { "slug": 1 })
                .options(
                    IndexOptions::builder()
                        .unique(true)
                        .sparse(true)
                        .name("orgs_slug_unique".to_string())
                        .build(),
                )
                .build(),
        )
        .await?;

        let reservations: Collection<OrgSlugReservation> =
            self.database.collection("slug_reservations");
        reservations
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "slug": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("slug_reservations_slug_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        reservations
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .name("slug_reservations_ttl".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        reservations
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("slug_reservations_org_id".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        let users: Collection<DashboardUser> = self.database.collection("dashboard_users");
        users
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "email": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("dashboard_users_email_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        // Drop the legacy `dashboard_users.org_id` index from the 1:1 era. The
        // field has been removed from the document; the index becomes stale on
        // upgrade and would only sit there indexing nothing.
        if let Err(err) = users.drop_index("dashboard_users_org_id").await {
            // `IndexNotFound` is the common case on fresh databases; log and continue.
            tracing::debug!(?err, "dashboard_users_org_id drop_index ignored");
        }

        let memberships: Collection<Membership> = self.database.collection("dashboard_memberships");
        memberships
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "org_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("dashboard_memberships_user_org_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        memberships
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("dashboard_memberships_org_id".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        let join_requests: Collection<JoinRequest> = self.database.collection("join_requests");
        // Partial unique on `pending` only — same (user_id, org_id) can have
        // multiple terminal-state rows (rejected, cancelled, approved) for
        // audit, but only one in-flight pending.
        join_requests
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "org_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .partial_filter_expression(doc! { "status": "pending" })
                            .name("join_requests_pending_user_org_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        join_requests
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "status": 1, "requested_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("join_requests_org_status_requested_at".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        join_requests
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "status": 1, "requested_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("join_requests_user_status_requested_at".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        let sessions: Collection<DashboardSession> = self.database.collection("dashboard_sessions");
        sessions
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .name("dashboard_sessions_ttl".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        let app_users: Collection<AppUser> = self.database.collection("app_users");
        // The username-uniqueness index must be PARTIAL: external shadow users
        // carry no `username_lower`, and a non-partial unique index would treat
        // the missing field as null and reject a second external user per Org.
        // Older deployments have the non-partial index under the same name —
        // drop it first (ignored when absent) so the partial version applies.
        let _ = app_users.drop_index("app_users_org_username_unique").await;
        app_users
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "username_lower": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("app_users_org_username_unique".to_string())
                            .partial_filter_expression(
                                doc! { "username_lower": { "$type": "string" } },
                            )
                            .build(),
                    )
                    .build(),
            )
            .await?;
        // External shadow users are unique per Org on `external_key`; partial so
        // internal users (no `external_key`) are excluded from the constraint.
        app_users
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "external_key": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("app_users_org_external_key_unique".to_string())
                            .partial_filter_expression(
                                doc! { "external_key": { "$type": "string" } },
                            )
                            .build(),
                    )
                    .build(),
            )
            .await?;
        app_users
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("app_users_org_id".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        let app_sessions: Collection<AppSession> = self.database.collection("app_sessions");
        app_sessions
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .name("app_sessions_ttl".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        app_sessions
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "app_user_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("app_sessions_app_user_id".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        let removed: Collection<RemovedMembership> =
            self.database.collection("removed_memberships");
        removed
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "email": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("removed_memberships_org_email_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        removed
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "cooldown_until": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .name("removed_memberships_ttl".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        // checkin_events: paginate per-AppUser by client time (mobile + admin
        // history) and per-Org by client time (future cross-Org reports).
        let checkin_events: Collection<CheckinEvent> = self.database.collection("checkin_events");
        checkin_events
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "app_user_id": 1, "occurred_at_client": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("checkin_events_user_client_time".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        checkin_events
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "occurred_at_client": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("checkin_events_org_client_time".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        // Backs the `legacy_backfill` example script's idempotent upsert: the
        // same legacy source document can be re-processed on every re-run
        // without producing a duplicate row. Partial so live-submitted events
        // (no `legacy_source_id`) are excluded from the constraint.
        checkin_events
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "legacy_source_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("checkin_events_legacy_source_id_unique".to_string())
                            .partial_filter_expression(
                                doc! { "legacy_source_id": { "$exists": true } },
                            )
                            .build(),
                    )
                    .build(),
            )
            .await?;

        // checkin_user_status: `_id` is the AppUser id, so uniqueness is
        // intrinsic. The secondary index is `(org_id, status)` for the live
        // board and the state-lock count.
        let checkin_status: Collection<CheckinUserStatus> =
            self.database.collection("checkin_user_status");
        checkin_status
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "status": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("checkin_user_status_org_status".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        // location_pings: per-AppUser pagination, per-Org export. No TTL —
        // see `location-tracking` spec's "Location pings are persisted with
        // dual timestamps" requirement: retention was previously a 90-day
        // TTL on `occurred_at_server`, removed so legacy-imported path data
        // (see `legacy_backfill` example script) isn't deleted on arrival.
        // Retention is unbounded pending a future rotation mechanism.
        let location_pings: Collection<LocationPing> = self.database.collection("location_pings");
        location_pings
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "app_user_id": 1, "occurred_at_client": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("location_pings_user_client_time".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        location_pings
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "occurred_at_client": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("location_pings_org_client_time".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        // Drop the old 90-day TTL index from deployments that predate this
        // change — `create_index` alone won't remove an index that's no
        // longer declared here. `IndexNotFound` is the common case on fresh
        // databases; log and continue (same pattern as
        // `dashboard_users_org_id` above).
        if let Err(err) = location_pings.drop_index("location_pings_ttl").await {
            tracing::debug!(?err, "location_pings_ttl drop_index ignored");
        }
        // Backs the `legacy_backfill` example script's idempotent upsert —
        // see the matching index on `checkin_events` above.
        location_pings
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "legacy_source_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("location_pings_legacy_source_id_unique".to_string())
                            .partial_filter_expression(
                                doc! { "legacy_source_id": { "$exists": true } },
                            )
                            .build(),
                    )
                    .build(),
            )
            .await?;

        // org_api_tokens: `token_hash` is the auth-path lookup key and must be
        // unique; `org_id` backs the admin-web list query.
        let org_api_tokens: Collection<OrgApiToken> = self.database.collection("org_api_tokens");
        org_api_tokens
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "token_hash": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("org_api_tokens_token_hash_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        org_api_tokens
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1 })
                    .options(
                        IndexOptions::builder()
                            .name("org_api_tokens_org_id".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        // password_reset_tokens: `token_hash` is the auth-path lookup key
        // (unique — collisions are astronomically unlikely with 256-bit
        // tokens, but the index still documents the intended invariant);
        // `user_id` backs the cooldown check (`find_latest_for_user`); TTL
        // on `expires_at` cleans up stale rows automatically, same pattern
        // as `dashboard_sessions`/`app_sessions`.
        let password_reset_tokens: Collection<PasswordResetToken> =
            self.database.collection("password_reset_tokens");
        password_reset_tokens
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "token_hash": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("password_reset_tokens_token_hash_unique".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        password_reset_tokens
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "user_id": 1, "created_at": -1 })
                    .options(
                        IndexOptions::builder()
                            .name("password_reset_tokens_user_created_at".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;
        password_reset_tokens
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "expires_at": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(0))
                            .name("password_reset_tokens_ttl".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        Ok(())
    }
}
