## Why

The current data model hard-wires a dashboard user to exactly one Org via `dashboard_users.org_id`, with `role` baked into the same row. That makes basic real-world cases impossible to express: the same person owning two Orgs they founded, an admin moonlighting as a member of a friend's Org, or a user staying logged in after leaving an Org while they pick a new one. It also makes future work — owner transfer, Org deletion, multi-Org admins — needlessly painful, since each one has to fight the 1:1 assumption.

Refactoring the user-Org relationship to many-to-many now (still pre-launch, no production data) is dramatically cheaper than later, and it unblocks several queued ROADMAP items. While we're touching this surface we also fold in **owner transfer**, since a multi-Org world makes ownership a first-class operation: the moment a user can hold multiple memberships, "transfer ownership and leave" becomes the obvious escape hatch for an owner who wants out.

## What Changes

- **BREAKING**: Introduce a new `dashboard_memberships` collection holding `(user_id, org_id, role, joined_at)`. `dashboard_users` loses `org_id` and `role`; it becomes pure identity (email + password_hash + timestamps).
- **BREAKING**: `dashboard_sessions.org_id` is renamed `current_org_id` and becomes mutable (the user can switch active Org during the session lifetime). It MAY also be `null` for users with zero memberships.
- New legitimate state: a logged-in user with **zero memberships**. `GET /me` returns `{ user, memberships: [], current_org: null }`. Org-scoped endpoints respond with a new `NO_ACTIVE_ORG` error.
- New endpoints for operating across memberships:
  - `POST /me/orgs` — logged-in user creates a brand-new Org and becomes its owner (no re-registration).
  - `POST /me/memberships` — logged-in user joins an existing Org via `org_code` / slug (no re-registration).
  - `POST /me/current-org` — switch the session's `current_org_id` to another Org the user is a member of.
- **BREAKING**: `POST /me/leave` is rescoped to leave **only** `current_org`. The user identity, sessions pointing at other Orgs, and other memberships are preserved. Sessions whose `current_org_id` equals the left Org are force-deleted.
- **BREAKING**: Admin removal (`DELETE /dashboard-users/:id`) deletes only the membership row in the caller's Org and the target's sessions for that Org. The target's user identity, other memberships, and sessions for other Orgs survive.
- **BREAKING**: `register mode=create|join` strictly requires a new email. Existing identities re-using `register` are rejected with `EMAIL_TAKEN`; they must log in and use `/me/orgs` or `/me/memberships`.
- Login picks `current_org_id` deterministically: prefer the oldest Org the user owns; otherwise the oldest membership; otherwise `null`. Frontend persists the user's last selected Org in localStorage and switches via `POST /me/current-org` on subsequent visits.
- Auth middleware re-queries membership on every request to compute `role` (no role caching on session row). Stale sessions (membership gone) are treated as `UNAUTHORIZED`.
- New endpoint **owner transfer**: `POST /orgs/me/owner` with `{ new_owner_user_id, current_password }`. Caller must be the current owner; target must already be `admin` of the same Org; `current_password` is re-verified. Effect: `Org.owner_id` changes; the previous owner becomes a regular admin (now demotable / leavable). No transfer cooldown.
- Owner cannot self-leave (rule preserved): the owner must transfer ownership first, then leave as a regular admin.
- Cooldown semantics unchanged: `removed_memberships` is still keyed by `(org_id, lowercase(email))`. The check moves from "during register" to "during any membership creation" so that `POST /me/memberships` is also gated.

Out of scope (covered by other ROADMAP items, not this change):

- AppUser remains 1:1 to an Org. Multi-Org for AppUser is a separate decision.
- Email verification on register, invite-link admin approval, `delete-org`.

## Capabilities

### New Capabilities

(none — the membership concept is captured as additions to `dashboard-auth`, which already governs role, removal, self-leave, and cooldown)

### Modified Capabilities

- `dashboard-auth`: Identity vs membership are split. Role lives on membership. `register` becomes new-identity-only. New endpoints for create-Org / join-Org / switch-Org as a logged-in user. `/me/leave` and admin removal rescoped to a single membership. `/me` payload changes shape. New `NO_ACTIVE_ORG` error class. Session carries mutable `current_org_id`.
- `org-tenancy`: Owner is no longer permanent — a new owner-transfer requirement is added. The "set once and never changed" wording is replaced with "set at creation; change only via transfer endpoint". Owner protections (cannot demote, cannot self-leave, cannot be removed) remain, now enforced in terms of `Org.owner_id` against the membership row.

## Impact

- **Schema**: new `dashboard_memberships` collection with unique index on `(user_id, org_id)`. `dashboard_users` loses `org_id` + `role` fields and the index on `org_id`. `dashboard_sessions.org_id` renamed to `current_org_id` (nullable). Pre-launch with no production data, so the migration story is "drop and recreate".
- **API code**: `domain.rs`, all repos in `db/`, all auth handlers, `me.rs`, `users.rs`, `orgs.rs`, and the auth middleware change. New repo `db/dashboard_memberships.rs`. New handlers under `me.rs` and `orgs.rs`.
- **API tests**: all 16 existing integration tests assume 1:1 and need rewriting. New tests for switching, joining as logged-in user, owner transfer, and zero-Org state.
- **admin-web**: `useAuth` composable rewritten to expose `memberships` + `current_org` + a `switch` action. New Org switcher in the header, new empty-state page for zero-Org users, new "create Org" / "join Org" flows reachable from the empty state and the switcher. Existing pages keep working but read `current_org` instead of `org`.
- **ROADMAP downstream**: `transfer-org-ownership` is delivered by this change (deletable from the list). `delete-org` becomes simpler (cascade memberships, leave identities). Invite-link admin approval gains a natural home (`memberships.status = pending|active`).
- **Performance**: middleware adds one `dashboard_memberships` lookup per authenticated request. MVP-acceptable; revisit with caching if observed latency warrants.
