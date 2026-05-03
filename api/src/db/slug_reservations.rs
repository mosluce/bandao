use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;

use crate::domain::OrgSlugReservation;
use crate::error::ApiResult;

#[derive(Clone)]
pub struct OrgSlugReservationRepository {
    coll: Collection<OrgSlugReservation>,
}

#[derive(Debug)]
pub enum ReservationInsertError {
    Duplicate,
    Db(mongodb::error::Error),
}

impl From<mongodb::error::Error> for ReservationInsertError {
    fn from(err: mongodb::error::Error) -> Self {
        Self::Db(err)
    }
}

impl OrgSlugReservationRepository {
    pub fn new(coll: Collection<OrgSlugReservation>) -> Self {
        Self { coll }
    }

    pub async fn find_by_slug(&self, slug: &str) -> ApiResult<Option<OrgSlugReservation>> {
        Ok(self.coll.find_one(doc! { "slug": slug }).await?)
    }

    /// Insert a new active reservation. Returns `Duplicate` on unique-index collision (slug already taken
    /// by an active or in-grace reservation).
    pub async fn try_insert_active(
        &self,
        slug: &str,
        org_id: ObjectId,
    ) -> Result<OrgSlugReservation, ReservationInsertError> {
        let now = DateTime::now();
        let reservation = OrgSlugReservation {
            id: ObjectId::new(),
            slug: slug.to_string(),
            org_id,
            expires_at: None,
            created_at: now,
        };
        match self.coll.insert_one(&reservation).await {
            Ok(_) => Ok(reservation),
            Err(err) => {
                if is_duplicate_key(&err) {
                    Err(ReservationInsertError::Duplicate)
                } else {
                    Err(ReservationInsertError::Db(err))
                }
            }
        }
    }

    /// Move the org's currently active reservation for `slug` into grace period.
    /// Matches by both slug and org_id to avoid clobbering another org's reservation.
    /// Returns true if a row was updated.
    pub async fn move_to_grace(
        &self,
        slug: &str,
        org_id: ObjectId,
        expires_at: DateTime,
    ) -> ApiResult<bool> {
        let res = self
            .coll
            .update_one(
                doc! { "slug": slug, "org_id": org_id, "expires_at": null },
                doc! { "$set": { "expires_at": expires_at } },
            )
            .await?;
        Ok(res.matched_count > 0)
    }

    /// Best-effort delete a reservation by id. Used to roll back a failed SET orchestration.
    pub async fn delete_by_id(&self, id: ObjectId) -> ApiResult<()> {
        self.coll.delete_one(doc! { "_id": id }).await?;
        Ok(())
    }
}

fn is_duplicate_key(err: &mongodb::error::Error) -> bool {
    use mongodb::error::ErrorKind;
    if let ErrorKind::Write(write_failure) = err.kind.as_ref() {
        if let mongodb::error::WriteFailure::WriteError(we) = write_failure {
            return we.code == 11000;
        }
    }
    false
}
