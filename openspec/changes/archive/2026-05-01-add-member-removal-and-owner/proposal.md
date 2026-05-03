## Why

The MVP currently has no way for an Org to evict a member nor for a member to leave on their own — once a `dashboard_user` is created, it lives forever. This blocks normal lifecycle scenarios (employee leaves the company, accidental member added via shared invite link). At the same time, the existing `LAST_ADMIN` rule is a fragile invariant: it protects against zero-admin Orgs only by counting admins at write time, with no anchor identifying the Org's true owner. Introducing an explicit `Org.owner_id` lets us replace the count-based rule with a structural one and unlocks a clean place to hang removal-protection logic.

## What Changes

- Add `owner_id: ObjectId` to `Org`, set to the registering user during `mode=create`. Owner is permanent for this change (transfer / Org delete are out of scope).
- Add `DELETE /dashboard-users/:id` (admin only) to evict another user from the Org. Caller cannot target themselves via this endpoint or the owner of the Org.
- Add `POST /me/leave` for any authenticated user to self-leave the Org. Owner cannot self-leave.
- Add `removed_memberships` collection plus `GET /dashboard-users/cooldowns` and `DELETE /dashboard-users/cooldowns/:email` to manage a 7-day rejoin cooldown per (org, email). Cooldown blocks `register mode=join` against the same Org with the same email until expiry; admins can clear it early.
- **BREAKING**: Remove the `LAST_ADMIN` invariant and the corresponding error code. Owner is permanent and always `role=admin`, so an Org always has at least one admin by construction. The role-update endpoint also gains a new rule: owner cannot be demoted to member.
- Add new error codes `OWNER_PROTECTED` (403) and `EMAIL_IN_COOLDOWN` (409). Remove `LAST_ADMIN` (409).
- Removal action is hard delete: `dashboard_user` row + all that user's `dashboard_sessions` + write `removed_memberships` marker, in that order.

## Capabilities

### New Capabilities

(none — all changes extend existing capabilities)

### Modified Capabilities

- `dashboard-auth`: adds member removal, self-leave, rejoin cooldown enforcement on register, owner-permanent rule on role updates; removes the last-admin invariant and its error code.
- `org-tenancy`: Org gains a permanent `owner_id` set on creation; the spec gains an Org-owner identity requirement.

## Impact

- **API code (`api/`)**: new handlers in `handlers/users.rs` and a new `handlers/me.rs` route; new `db/removed_memberships.rs` repository; `db/orgs.rs` and `domain.rs` gain `owner_id`; `handlers/auth.rs` `register` writes owner on create and consults cooldown on join; `error.rs` adds two error variants and removes `LastAdmin`.
- **MongoDB**: new `removed_memberships` collection with unique index `{org_id, email}` and TTL index on `cooldown_until`; `orgs` gains `owner_id` field (no index needed for MVP — only read by id lookup).
- **admin-web**: user list page gains remove buttons (hidden for owner row and self row), profile page gains a Danger zone with Leave Org (disabled for owner), new cooldown management subpage.
- **Migration**: none — dev DB is wiped before deploying this change.
- **Future ROADMAP items unblocked**: `transfer-org-ownership` (relocate `owner_id`), `delete-org` (cascade Org + its users).
