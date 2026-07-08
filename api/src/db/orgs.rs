use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;

use crate::domain::{DEFAULT_ORG_TIMEZONE, ExternalAuthConfig, Org, OrgAuthSource};
use crate::error::{ApiError, ApiResult};

#[derive(Clone)]
pub struct OrgRepository {
    coll: Collection<Org>,
}

impl OrgRepository {
    pub fn new(coll: Collection<Org>) -> Self {
        Self { coll }
    }

    pub async fn create(&self, name: &str, code: &str, owner_id: ObjectId) -> ApiResult<Org> {
        let now = DateTime::now();
        // New Orgs default to transfer_enabled=true (the more permissive
        // option) and Asia/Taipei. Both can be flipped via PATCH /orgs/me/settings.
        let mut settings = bson::Document::new();
        settings.insert("checkin", doc! { "transfer_enabled": true });
        let org = Org {
            id: ObjectId::new(),
            name: name.to_string(),
            code: code.to_string(),
            owner_id,
            slug: None,
            slug_changed_at: None,
            timezone: DEFAULT_ORG_TIMEZONE.to_string(),
            settings,
            created_at: now,
            updated_at: now,
        };
        self.coll.insert_one(&org).await?;
        Ok(org)
    }

    /// Apply a partial settings patch — `transfer_enabled` and
    /// `location_tracking_enabled` land under `settings.checkin.*`;
    /// `timezone` is a top-level field. State-lock and timezone validation
    /// are handler-side concerns; this method just runs the writes. Returns
    /// the updated Org.
    pub async fn update_settings(
        &self,
        id: ObjectId,
        transfer_enabled: Option<bool>,
        timezone: Option<&str>,
        location_tracking_enabled: Option<bool>,
    ) -> ApiResult<Org> {
        let now = DateTime::now();
        let mut set = doc! { "updated_at": now };
        if let Some(tz) = timezone {
            set.insert("timezone", tz);
        }
        if let Some(flag) = transfer_enabled {
            set.insert("settings.checkin.transfer_enabled", flag);
        }
        if let Some(flag) = location_tracking_enabled {
            set.insert("settings.checkin.location_tracking_enabled", flag);
        }
        let result = self
            .coll
            .find_one_and_update(doc! { "_id": id }, doc! { "$set": set })
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Set `settings.auth_source` and, when provided, replace
    /// `settings.external_auth`. Switching to `internal` leaves any stored
    /// `external_auth` config intact (so switching back restores it) unless a
    /// new config is supplied.
    pub async fn set_auth_config(
        &self,
        id: ObjectId,
        auth_source: OrgAuthSource,
        external_auth: Option<&ExternalAuthConfig>,
    ) -> ApiResult<Org> {
        let now = DateTime::now();
        let source_str = match auth_source {
            OrgAuthSource::Internal => "internal",
            OrgAuthSource::ExternalDb => "external_db",
        };
        let mut set = doc! {
            "updated_at": now,
            "settings.auth_source": source_str,
        };
        if let Some(cfg) = external_auth {
            let cfg_doc = bson::to_document(cfg)?;
            set.insert("settings.external_auth", cfg_doc);
        }
        let result = self
            .coll
            .find_one_and_update(doc! { "_id": id }, doc! { "$set": set })
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    pub async fn find_by_id(&self, id: ObjectId) -> ApiResult<Option<Org>> {
        Ok(self.coll.find_one(doc! { "_id": id }).await?)
    }

    pub async fn find_by_code(&self, code: &str) -> ApiResult<Option<Org>> {
        Ok(self.coll.find_one(doc! { "code": code }).await?)
    }

    pub async fn find_by_slug(&self, slug: &str) -> ApiResult<Option<Org>> {
        Ok(self.coll.find_one(doc! { "slug": slug }).await?)
    }

    pub async fn set_slug(
        &self,
        id: ObjectId,
        slug: &str,
        slug_changed_at: DateTime,
    ) -> ApiResult<Org> {
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { "slug": slug, "slug_changed_at": slug_changed_at, "updated_at": slug_changed_at } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    pub async fn clear_slug(&self, id: ObjectId, slug_changed_at: DateTime) -> ApiResult<Org> {
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! {
                    "$set": { "slug_changed_at": slug_changed_at, "updated_at": slug_changed_at },
                    "$unset": { "slug": "" }
                },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    pub async fn delete_by_id(&self, id: ObjectId) -> ApiResult<()> {
        self.coll.delete_one(doc! { "_id": id }).await?;
        Ok(())
    }

    /// Replace the org's code with `new_code`. Returns `NotFound` if the org id is unknown.
    pub async fn rotate_code(&self, id: ObjectId, new_code: &str) -> ApiResult<Org> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { "code": new_code, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }

    /// Transfer ownership: set `owner_id` to `new_owner_id` and bump
    /// `updated_at`. Caller is expected to have already validated that
    /// `new_owner_id` is currently an admin of this Org.
    pub async fn transfer_owner(&self, id: ObjectId, new_owner_id: ObjectId) -> ApiResult<Org> {
        let now = DateTime::now();
        let result = self
            .coll
            .find_one_and_update(
                doc! { "_id": id },
                doc! { "$set": { "owner_id": new_owner_id, "updated_at": now } },
            )
            .return_document(mongodb::options::ReturnDocument::After)
            .await?;
        result.ok_or(ApiError::NotFound)
    }
}
