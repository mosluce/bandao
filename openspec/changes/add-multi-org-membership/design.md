## Context

Argus today encodes the user-Org relationship inside the user document itself: `dashboard_users.org_id` and `dashboard_users.role` make every user belong to exactly one Org with one role. The session row carries the same `org_id`, baked in at login. The auth middleware reads `(user_id, org_id, role)` straight off the session and hands them to handlers as `AuthContext`.

This is fine for a strict tenant-isolation MVP, but it forecloses several near-term features (owner transfer, multi-Org admins, "I left but want to log in and look around") and forces awkward workarounds: leaving an Org means deleting the user identity and its sessions, even though the email + password are perfectly reusable for joining any other Org.

The change is foundational: it splits **identity** (who you are) from **membership** (which Orgs you are in, and as what). It also delivers **owner transfer**, which is small once memberships exist and unblocks `transfer-org-ownership` from ROADMAP.

We are pre-launch with no production data, so the migration cost is concentrated in code (api + admin-web) and tests. Existing 16 integration tests assume 1:1 and need rewriting.

## Goals / Non-Goals

**Goals:**

- Allow a single dashboard identity to hold 0..N memberships, each with its own role.
- Express ownership in a way that survives transfer.
- Keep the existing tenant-isolation guarantees intact: a user without a membership for Org X cannot see or affect Org X data, regardless of their other memberships.
- Preserve the existing session model (cookie-based, opaque token, sliding expiry).
- Allow zero-Org state to be a legitimate, navigable UI state — not a bug.
- Deliver `transfer-org-ownership` requirement inside this change; remove it from ROADMAP.

**Non-Goals:**

- Multi-Org for AppUser (deferred; AppUser stays 1:1).
- Email verification, invite-link admin approval, Org deletion — separate ROADMAP items.
- Per-tab session isolation (all tabs in one browser share a session and `current_org_id`).
- Caching membership lookups in the auth middleware (revisit when we have latency data).
- Audit log of ownership transfers (MVP records only the latest `Org.owner_id`).

## Decisions

### Data model: separate `dashboard_memberships` collection (not embedded array on user)

```
dashboard_users                dashboard_memberships              orgs
  _id                            _id                               _id
  email (unique)                 user_id ──────┐                   name
  password_hash                  org_id ───────┼──▶ orgs           code
  created_at                     role          │                   owner_id ──┐
  updated_at                     joined_at     │                   slug       │
                                 updated_at    │                   ...        │
                                               └──▶ dashboard_users ◀─────────┘
                                 unique (user_id, org_id)

dashboard_sessions               removed_memberships
  _id (token)                      _id
  user_id                          org_id
  current_org_id (nullable) ▲      email (lowercased)
  expires_at                       removed_at
  created_at                       cooldown_until
  ▲                                removal_kind
  └─ mutable; force-deleted        unique (org_id, email)
     when membership for
     (user_id, current_org_id)
     is removed
```

**Why a separate collection over an embedded `memberships: [...]` array on `dashboard_users`:**

- Atomic insert/delete of one membership without rewriting the user doc.
- Clean unique index on `(user_id, org_id)` to prevent duplicate memberships.
- Querying "all members of Org X" is a single index hit on `org_id`, not a scan of every user.
- Future per-membership fields (e.g. `pending` status for invite-approval, `last_used_at` for default-Org rule v2) are easy to add without bloating the user doc.

Trade-off: middleware does one extra lookup per request to resolve `role`. Acceptable for MVP; revisit with caching later if needed.

### Auth context resolution: query membership every request

The auth middleware does:

1. Look up session by token → get `(user_id, current_org_id, expires_at)`.
2. If `current_org_id` is null → request is allowed only on org-agnostic endpoints (`/me`, `/me/orgs`, `/me/memberships`, `/me/current-org`, `/auth/logout`); org-scoped endpoints respond `NO_ACTIVE_ORG`.
3. If `current_org_id` is set → look up `dashboard_memberships(user_id, current_org_id)` to get `role`.
4. If membership not found (data race or stale state) → treat session as `UNAUTHORIZED` (force the client to re-login).

Alternative considered: cache `role` on the session row. Rejected for now because role changes (admin promote/demote) would require finding and updating every session of the affected user. Not impossible, but extra failure mode for a hot path that one indexed lookup handles cleanly.

### Session lifecycle: force-kick on membership delete

When a membership for `(user_id, org_id)` is deleted (via `/me/leave`, admin removal, or — in future — Org deletion):

- Delete every `dashboard_sessions` row where `user_id = user_id AND current_org_id = org_id`.
- Sessions where `current_org_id` points at *other* Orgs the user is still a member of survive untouched.

This means a user who is admin in Org A and Org B, looking at Org A in tab 1 and Org B in tab 2, gets kicked from Org A only when they leave Org A; tab 2 stays alive.

If the user has zero remaining memberships after the leave, every session that was pointing at the left Org dies. The user can log in again and land in the zero-Org state.

### Default `current_org_id` on login

```
priority order (highest first):
  1. Oldest Org the user owns (smallest `org.created_at` among orgs where org.owner_id == user._id)
  2. Oldest membership (smallest `membership.joined_at`)
  3. null  (zero memberships)
```

The frontend layers UX preference on top: it persists `lastSelectedOrgId` in localStorage and, after login, calls `POST /me/current-org` if that Org is in the user's current memberships. The server-side default is what the user sees if they have no localStorage entry (first device, cleared cache, etc.).

Alternative considered: store `default_org_id` on the user record and let users set it explicitly. Deferred — adds an endpoint + UI for a preference most users won't think about. The owned-first / oldest-fallback rule covers the common case.

### Register: strict separation of identities

`POST /auth/register` (both `mode=create` and `mode=join`) continues to reject any request whose email already exists in `dashboard_users` with `EMAIL_TAKEN`. Existing identities cannot use `register` to add a membership; they must log in and use `POST /me/memberships` (join) or `POST /me/orgs` (create).

Alternative considered: "register with existing email + correct password = login + add membership". Rejected — it conflates two operations and forces the registration handler to verify password, which subtly changes its security profile (currently registration is unauthenticated; making it sometimes-authenticated is a footgun).

### Owner transfer: `POST /orgs/me/owner` with password re-auth

```
POST /orgs/me/owner
Body: { new_owner_user_id: ObjectId, current_password: string }

Preconditions:
  - caller is the current owner (org.owner_id == ctx.user_id)
  - new_owner has an active admin membership in the same org
  - new_owner is not the caller (no self-transfer)
  - current_password verifies against caller's stored password_hash

Effect:
  - org.owner_id ← new_owner_user_id
  - org.updated_at ← now
  - both old and new owner remain admin (their membership rows are unchanged)
  - the previous owner is now a regular admin: demotable, removable, can self-leave
  - the new owner is now protected by the existing owner-protection rules
  - sessions, cooldowns, and slug state are not touched

Errors:
  - FORBIDDEN if caller is not the owner
  - INVALID_PASSWORD if password verification fails
  - INVALID_TARGET if new_owner is not an admin in this org
  - SAME_OWNER if new_owner_user_id == ctx.user_id
```

Decisions on the edges:

- **Target must already be admin** (not member). Promote-then-transfer is two clear steps; one-step transfer-with-promote was rejected to keep the state machine small.
- **Password re-auth, not 2FA.** MVP doesn't have 2FA infrastructure. Re-asking for the current password is the standard cheap re-auth and aligns with how `delete-org` will likely be gated later.
- **No transfer cooldown.** Transfers are deliberate, owner-initiated actions; unlike slug changes, there's no DDoS / squatting vector here. Add a cooldown later only if a real abuse case appears.
- **No history.** MVP records only the latest `owner_id`. An audit log of past owners is a separate concern.

### Owner cannot self-leave (preserved)

`POST /me/leave` continues to reject the owner. The owner's path out of an Org is: transfer ownership first, then leave as a regular admin.

Alternative considered: a "leave + transfer to" combined endpoint. Rejected — overloads `/me/leave` with two semantics and complicates the error space. Keep the two operations atomic and separately observable.

### Cooldown semantics: keyed unchanged, check-point widened

`removed_memberships` is still keyed by `(org_id, lowercase(email))` with a 7-day cooldown. The change is **where** the check fires:

- **Before**: only `register mode=join` checked the cooldown.
- **After**: any path that creates a membership checks the cooldown — `register mode=join` AND `POST /me/memberships`. This way an existing identity can't bypass cooldown by logging in and using the new membership endpoint.

`register mode=create` is not affected — creating a brand-new Org cannot collide with a cooldown for that Org (the Org didn't exist yet).

### Email is normalized once, on user-identity creation

The user-identity record stores email in its original-cased form (as today), and queries use case-insensitive lookups. The cooldown collection continues to lower-case the stored email key as it does today. This change does not alter email-normalization rules.

## Risks / Trade-offs

- **Per-request membership lookup → latency** → MVP-acceptable; one indexed lookup. Mitigation if it bites: copy `role` into session row at switch/login time, invalidate on role change.
- **Stale-session race** (membership deleted between session lookup and membership lookup) → middleware returns `UNAUTHORIZED` so the client re-logs in. Force-kick on membership delete keeps this edge case small.
- **Frontend state reactivity** — every page that displays Org-scoped data must re-fetch when `current_org_id` changes. We standardize this through `useAuth().currentOrgId` reactivity and document the invalidation pattern in the composable.
- **Test rewrite cost** — 16 existing integration tests rely on 1:1. All need rewriting. Mitigated by the test patterns being short and consistent today; the bulk is repetitive.
- **Owner transfer bypassing existing protections** — only the current owner can call the endpoint, target must already be admin, password re-auth required. These three combined match the threat model of a hostile-but-authenticated session.
- **Default-org rule changes over time** — the rule lives on the server and frontend localStorage takes precedence. If we later add a user-level preference, the server rule becomes the fallback only. Backwards compatible.

## Migration Plan

Pre-launch, no production data. The plan is "drop and recreate":

1. Drop `dashboard_users.org_id` and `dashboard_users.role` fields and the `org_id` index.
2. Rename `dashboard_sessions.org_id` → `dashboard_sessions.current_org_id` (now nullable).
3. Create `dashboard_memberships` collection with unique index on `(user_id, org_id)` and secondary index on `org_id` (for "list members of Org X").
4. Local dev databases get wiped during the change. Document this in the proposal/PR description so collaborators don't expect their local accounts to survive the upgrade.

No rollback strategy beyond "revert the commit and recreate the local DB" — pre-launch, this is acceptable.

## Open Questions

None blocking implementation. The following are deferred but worth noting:

- Whether to add `last_used_at` to `dashboard_memberships` later, to drive smarter default-Org selection. Not needed for MVP.
- Whether `DELETE /orgs/me/owner` should ever be a thing (relinquish ownership without naming a successor → Org enters "ownerless" state). Currently no — Orgs always have exactly one owner.
- Whether owner-transfer should also clear the old owner's session and require re-login. Deciding "no" for MVP — the old owner is now a regular admin and that role change happens transparently on next request via the usual middleware path.
