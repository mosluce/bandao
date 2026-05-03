use bson::DateTime;
use bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Org {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub name: String,
    pub code: String,
    pub owner_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug_changed_at: Option<DateTime>,
    #[serde(default)]
    pub settings: bson::Document,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgSlugReservation {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub slug: String,
    pub org_id: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime>,
    pub created_at: DateTime,
}

/// Pure identity record. The user's Org affiliations live in
/// `dashboard_memberships`, not on this row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardUser {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

/// One row per (user, org) pair. Carries the user's role in that org.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub user_id: ObjectId,
    pub org_id: ObjectId,
    pub role: Role,
    pub joined_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSession {
    #[serde(rename = "_id")]
    pub token: String,
    pub user_id: ObjectId,
    /// The Org this session is currently scoped to. Mutable across the session
    /// lifetime via `POST /me/current-org`. May be `None` for users with zero
    /// memberships, or whose memberships were all removed mid-session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_org_id: Option<ObjectId>,
    pub expires_at: DateTime,
    pub created_at: DateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RemovalKind {
    Kicked,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemovedMembership {
    #[serde(rename = "_id")]
    pub id: ObjectId,
    pub org_id: ObjectId,
    pub email: String,
    pub removed_at: DateTime,
    pub cooldown_until: DateTime,
    pub removal_kind: RemovalKind,
}
