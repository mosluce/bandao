## Context

argus currently models dashboard membership as a one-to-one binding: each `dashboard_user` carries `org_id` and `role` directly, and the `LAST_ADMIN` invariant is the sole guard against producing an Org with zero admins. There is no eviction, no self-leave, and no concept of an Org owner separate from "any admin."

This change introduces three coupled mechanics:

1. **Org owner** — a permanent anchor identifying who created the Org.
2. **Membership lifecycle** — admin removal of others, self-leave for everyone else, with hard delete and session cascade.
3. **Rejoin cooldown** — a 7-day per-(org, email) lock to prevent immediate re-registration via shared invite codes.

The decisions below were converged in an `/opsx:explore` session. The single biggest constraint shaping the design: **owner is permanent for this change** — there is no transfer endpoint and no Org delete. This is acceptable only because argus is pre-launch; the limitation is captured below and tracked as separate ROADMAP items.

## Goals / Non-Goals

**Goals:**

- Let admins evict other dashboard users from their own Org.
- Let any non-owner dashboard user voluntarily leave their Org.
- Cleanly identify the Org's owner so removal protection has a structural anchor instead of a count-based invariant.
- Block immediate re-registration via shared invite code (`mode=join`) for 7 days, with admin override.
- Cascade-clean session state so a removed user is silently logged out on next request.

**Non-Goals:**

- Ownership transfer (admin → owner).
- Org deletion.
- Soft-delete / membership tombstones beyond the cooldown marker (the user record is hard-deleted).
- Decoupling user identity from Org membership (i.e., a user without any Org).
- Notifying removed users by email or in-app.
- Cross-Org cooldown (cooldowns are per Org).
- AppUser lifecycle (separate ROADMAP item).
- Migration script for existing Orgs (dev DB will be wiped before this ships).

## Decisions

### Owner stored as `Org.owner_id`, not a new role

```
Chosen: Org { ..., owner_id: ObjectId }    + Role unchanged (Admin | Member)
Rejected:
  - new Role::Owner variant (touches every existing role check, complicates update_role)
  - is_creator boolean on dashboard_user (no transfer story, scattered checks)
  - implicit "earliest admin" (forces extra query, can't transfer)
```

`owner_id` lives on the `Org` document because ownership is a property of the Org, not the user — there is exactly one owner per Org, multiple Orgs may share an admin pool only after future user/membership decoupling. Putting it on the Org also makes `RequireOwner`-style checks a single field comparison after the existing org load.

### Owner is coupled to `role=admin`

Rejected the "independent" model where `owner_id` and `role` are orthogonal because:

- Owner-without-admin-role has no clean UX placement (do they see the admin nav? the user list?). Forcing owner → admin removes the dead state.
- Last-admin-zero-Org becomes structurally impossible: owner exists, owner is admin, therefore Org has ≥1 admin. The runtime `count_admins_in_org` check and `LAST_ADMIN` error are deleted, simplifying the role-update handler.
- Owner-protection rules collapse to a single check: `target.id == org.owner_id` → `OWNER_PROTECTED`.

Side effect: `update_role` rejects demoting the owner regardless of who initiates. Promotion of the owner via `update_role { role: "admin" }` is a no-op because the role is already admin.

### Hard delete + cascade in the API layer (not transactions)

```
Removal sequence:
  1. load target dashboard_user (validate org match + owner check)
  2. delete_many dashboard_sessions where user_id = target.id
  3. delete_one dashboard_users where _id = target.id
  4. upsert removed_memberships (org_id, lowercase(email))
```

MongoDB multi-document transactions require a replica set; the current dev compose uses a single-node Mongo. The order above is partial-failure tolerant:

- Crash after (2): user record still exists, sessions are already gone — admin can retry, or the user finds their cookie 401s at the middleware (which already revalidates user existence).
- Crash after (3): sessions and user are gone, but no marker — the cooldown silently doesn't apply. Acceptable risk given (a) the surface is small, (b) admin can manually re-add a marker via the cooldown API as soon as we expose it as an admin tool... actually we don't expose insert, only delete. Mitigation: we accept that a server crash during this window leaves no cooldown. If this ever bites in practice we can move to an "insert marker first, then delete" order; the trade-off is that an interrupted insert leaves a stale cooldown blocking an undeleted user.
- Crash after (4): clean.

We do (2)→(3)→(4) (sessions → user → marker) because the user-existence-implies-marker-may-not-exist gap is less harmful than the marker-exists-but-user-still-active gap (which would block a still-valid user from being re-invited).

### `removed_memberships` schema

```
removed_memberships {
  _id:            ObjectId (default)
  org_id:         ObjectId
  email:          String (lowercased)
  removed_at:     DateTime
  cooldown_until: DateTime         // = removed_at + 7d
  removal_kind:   "kicked" | "left"
}

Indexes:
  - { org_id: 1, email: 1 } unique          // dedupe + lookup on register
  - { cooldown_until: 1 } expireAfterSeconds: 0   // TTL auto-purge
```

Notes:

- The unique index means a second removal/leave for the same `(org_id, email)` while the first marker is alive will fail. This is correct — at any given moment there's at most one user with a given `(org_id, email)` (email is globally unique on `dashboard_users`), so the only way to hit this collision is if a marker survived past a successful re-registration via admin override. We resolve it by treating the cooldown clear as a delete (not a flag flip) and by having admin override delete the marker rather than expire it. The `register mode=join` path also doesn't touch markers — successful join leaves any expired marker for TTL to clean.
- Storing `email` lowercase decouples the index from any future case-insensitive policy on `dashboard_users.email` (which is currently stored as-entered, but compared case-sensitively — we should treat that as a known inconsistency and document it; not in scope here).
- Storing `removed_at` and `cooldown_until` separately (rather than computing one from the other) is for audit-trail readability when listing in the admin UI.

### Endpoint shape

```
DELETE /dashboard-users/:id              admin only
  • requires :id ≠ ctx.user_id (otherwise FORBIDDEN with hint)
  • requires :id ≠ ctx.org.owner_id (otherwise OWNER_PROTECTED)
  • requires target.org_id == ctx.org_id (otherwise NOT_FOUND)
  • returns 204

POST /me/leave                           any authenticated user
  • requires ctx.user_id ≠ ctx.org.owner_id (otherwise OWNER_PROTECTED)
  • clears session cookie, returns 204

GET  /dashboard-users/cooldowns          admin only
  • returns array scoped to ctx.org_id

DELETE /dashboard-users/cooldowns/:email admin only
  • idempotent: deletes marker if exists, 204 either way
```

Self-removal via `DELETE /dashboard-users/:id` was rejected because it muddies the audit story (kicked vs left) and requires per-row branch logic in the handler. Forcing `/me/leave` for self keeps `removal_kind` unambiguous.

### Cooldown override mechanism

The override is a separate admin-only DELETE on the marker rather than a flag on the register call. The latter doesn't work because `register` is a public endpoint and cannot authenticate the admin. The former gives us a natural admin-web management page (list + clear), reuses our existing `RequireAdmin` extractor, and is auditable.

### Error codes

```
Added:
  OWNER_PROTECTED       403   - operation cannot target the org owner
  EMAIL_IN_COOLDOWN     409   - this email cannot rejoin this org until cooldown expires

Removed:
  LAST_ADMIN            409   - subsumed by owner-permanent rule
```

`OWNER_PROTECTED` is 403 (the operation is forbidden by policy on a real, visible target) rather than 404 (which we use for cross-Org cases where the target should be invisible). The owner is necessarily visible to the caller because the caller is in the same Org.

`EMAIL_IN_COOLDOWN` is 409 (conflict with existing state) consistent with `EMAIL_TAKEN`.

## Risks / Trade-offs

- **Owner permanently locked into the Org** → Mitigation: documented limitation; ROADMAP items `transfer-org-ownership` and `delete-org` queued. Pre-launch users cannot get stuck because data is wipeable.
- **Hard delete loses any future audit trail** → Mitigation: argus has no `dashboard_user`-referencing collections yet (sessions are the only join, and we explicitly cascade them). When audit logging is introduced, denormalize identity onto each audit row rather than retroactively softening the delete.
- **Partial failure between delete-user and write-marker leaves no cooldown** → Mitigation: accepted; rare, recoverable manually if needed (admin can re-issue removal? — actually no, target is gone; in practice we'd extend the admin UI later to manually create a marker if abuse appears).
- **Email casing inconsistency between `dashboard_users.email` (raw) and `removed_memberships.email` (lowercased)** → Mitigation: documented; cooldown lookup must lowercase the input email at every call site. Future cleanup change can lowercase `dashboard_users.email` too.
- **`removed_memberships` unique index could conflict if cooldown override leaves a stale marker** → Mitigation: cooldown clear is a delete (not a flag); the unique index is desirable because it surfaces logic bugs immediately.
- **TTL precision on Mongo** → Mongo's TTL background task runs every 60 seconds, so `cooldown_until` enforcement at exactly the boundary is approximate. The application-level check at `register` handles the fine-grained boundary; TTL is purely housekeeping.

## Migration Plan

There is no migration in the database sense — dev DB will be wiped before this change deploys. Implementation order matters within the change:

1. Domain and DB layer first: `Org.owner_id`, `removed_memberships` repository, indexes.
2. Register-create wires up `owner_id`.
3. Removal / self-leave handlers + register-join cooldown check + role-update owner guard.
4. Remove `LAST_ADMIN` error variant and admin-count code.
5. admin-web UI changes consume the new endpoints.

Rollback strategy: revert the change set; dev DB wipe re-aligns schema.
