pub mod dashboard_memberships;
pub mod dashboard_sessions;
pub mod dashboard_users;
pub mod orgs;
pub mod removed_memberships;
pub mod slug_reservations;

use std::time::Duration;

use bson::doc;
use mongodb::options::{ClientOptions, IndexOptions};
use mongodb::{Client, Collection, Database, IndexModel};

use crate::domain::{
    DashboardSession, DashboardUser, Membership, Org, OrgSlugReservation, RemovedMembership,
};
use crate::error::ApiResult;

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

        Ok(())
    }
}
