use bson::oid::ObjectId;
use bson::{DateTime, doc};
use mongodb::Collection;
use mongodb::options::ReturnDocument;

use crate::domain::{ApiTokenScope, ApiTokenStatus, OrgApiToken};
use crate::error::ApiResult;

#[derive(Clone)]
pub struct OrgApiTokenRepository {
    coll: Collection<OrgApiToken>,
}

impl OrgApiTokenRepository {
    pub fn new(coll: Collection<OrgApiToken>) -> Self {
        Self { coll }
    }

    pub async fn insert(
        &self,
        org_id: ObjectId,
        name: &str,
        token_hash: &str,
        token_prefix: &str,
        scopes: Vec<ApiTokenScope>,
        created_by: ObjectId,
    ) -> ApiResult<OrgApiToken> {
        let token = OrgApiToken {
            id: ObjectId::new(),
            org_id,
            name: name.to_string(),
            token_hash: token_hash.to_string(),
            token_prefix: token_prefix.to_string(),
            scopes,
            status: ApiTokenStatus::Active,
            created_at: DateTime::now(),
            created_by,
            last_used_at: None,
            rotated_at: None,
        };
        self.coll.insert_one(&token).await?;
        Ok(token)
    }

    pub async fn list_by_org(&self, org_id: ObjectId) -> ApiResult<Vec<OrgApiToken>> {
        let mut cursor = self.coll.find(doc! { "org_id": org_id }).await?;
        let mut out = Vec::new();
        while cursor.advance().await? {
            out.push(cursor.deserialize_current()?);
        }
        Ok(out)
    }

    pub async fn find_by_id_and_org(
        &self,
        id: ObjectId,
        org_id: ObjectId,
    ) -> ApiResult<Option<OrgApiToken>> {
        Ok(self
            .coll
            .find_one(doc! { "_id": id, "org_id": org_id })
            .await?)
    }

    /// Auth-path lookup: active tokens only, matched by hash. Not org-scoped
    /// up front — the hash alone determines identity, same as session-token
    /// lookups elsewhere in `db/`.
    pub async fn find_active_by_hash(&self, token_hash: &str) -> ApiResult<Option<OrgApiToken>> {
        Ok(self
            .coll
            .find_one(doc! { "token_hash": token_hash, "status": "active" })
            .await?)
    }

    /// Best-effort last-used bump. Callers should log and continue on error
    /// rather than fail the request this ran alongside.
    pub async fn touch_last_used(&self, id: ObjectId) -> ApiResult<()> {
        self.coll
            .update_one(
                doc! { "_id": id },
                doc! { "$set": { "last_used_at": DateTime::now() } },
            )
            .await?;
        Ok(())
    }

    pub async fn update_status(
        &self,
        id: ObjectId,
        org_id: ObjectId,
        status: ApiTokenStatus,
    ) -> ApiResult<Option<OrgApiToken>> {
        let status_bson = bson::to_bson(&status)?;
        Ok(self
            .coll
            .find_one_and_update(
                doc! { "_id": id, "org_id": org_id },
                doc! { "$set": { "status": status_bson } },
            )
            .return_document(ReturnDocument::After)
            .await?)
    }

    /// Rotate: replace `token_hash`/`token_prefix`, stamp `rotated_at`.
    /// `name` and `scopes` are untouched — the caller keeps the same row,
    /// just a new secret.
    pub async fn rotate(
        &self,
        id: ObjectId,
        org_id: ObjectId,
        token_hash: &str,
        token_prefix: &str,
    ) -> ApiResult<Option<OrgApiToken>> {
        Ok(self
            .coll
            .find_one_and_update(
                doc! { "_id": id, "org_id": org_id },
                doc! {
                    "$set": {
                        "token_hash": token_hash,
                        "token_prefix": token_prefix,
                        "rotated_at": DateTime::now(),
                    }
                },
            )
            .return_document(ReturnDocument::After)
            .await?)
    }

    /// Hard-delete. Returns the number of documents removed (0 or 1) so
    /// callers can distinguish "not found / wrong org" from success.
    pub async fn delete(&self, id: ObjectId, org_id: ObjectId) -> ApiResult<u64> {
        let result = self
            .coll
            .delete_one(doc! { "_id": id, "org_id": org_id })
            .await?;
        Ok(result.deleted_count)
    }
}
