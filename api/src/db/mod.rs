pub mod app_sessions;
pub mod app_users;
pub mod checkin_events;
pub mod checkin_user_status;
pub mod dashboard_memberships;
pub mod dashboard_sessions;
pub mod dashboard_users;
pub mod location_pings;
pub mod orgs;
pub mod removed_memberships;
pub mod slug_reservations;

use std::time::Duration;

use bson::doc;
use mongodb::options::{ClientOptions, IndexOptions};
use mongodb::{Client, Collection, Database, IndexModel};

use crate::domain::{
    AppSession, AppUser, CheckinEvent, CheckinUserStatus, DashboardSession, DashboardUser,
    LocationPing, Membership, Org, OrgSlugReservation, RemovedMembership,
};
use crate::error::ApiResult;

pub use app_sessions::AppSessionRepository;
pub use app_users::{AppUserInsertError, AppUserRepository};
pub use checkin_events::CheckinEventRepository;
pub use checkin_user_status::{CheckinStatusInsertError, CheckinUserStatusRepository};
pub use location_pings::{InsertManyOutcome, LOCATION_PING_BATCH_MAX, LocationPingRepository};
pub use dashboard_memberships::{MembershipInsertError, MembershipRepository};
pub use dashboard_sessions::DashboardSessionRepository;
pub use dashboard_users::DashboardUserRepository;
pub use orgs::OrgRepository;
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
}

impl Db {
    pub async fn connect(uri: &str, db_name: &str) -> ApiResult<Self> {
        let mut options = ClientOptions::parse(uri).await?;
        options.app_name = Some("argus-api".to_string());
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
            database,
        })
    }

    pub async fn ensure_indexes(&self) -> ApiResult<()> {
        let orgs: Collection<Org> = self.database.collection("orgs");
        orgs.create_index(
            IndexModel::builder()
                .keys(doc! { "code": 1 })
                .options(IndexOptions::builder().unique(true).name("orgs_code_unique".to_string()).build())
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

        let memberships: Collection<Membership> =
            self.database.collection("dashboard_memberships");
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
        app_users
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "org_id": 1, "username_lower": 1 })
                    .options(
                        IndexOptions::builder()
                            .unique(true)
                            .name("app_users_org_username_unique".to_string())
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
        let checkin_events: Collection<CheckinEvent> =
            self.database.collection("checkin_events");
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

        // location_pings: per-AppUser pagination, per-Org export, plus a
        // 90-day TTL keyed on `occurred_at_server` so client-clock drift can't
        // skew retention. Mongo's TTL monitor runs ~every 60s, so retention is
        // "90 days ± a minute".
        let location_pings: Collection<LocationPing> =
            self.database.collection("location_pings");
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
        location_pings
            .create_index(
                IndexModel::builder()
                    .keys(doc! { "occurred_at_server": 1 })
                    .options(
                        IndexOptions::builder()
                            .expire_after(Duration::from_secs(90 * 24 * 3600))
                            .name("location_pings_ttl".to_string())
                            .build(),
                    )
                    .build(),
            )
            .await?;

        Ok(())
    }
}
