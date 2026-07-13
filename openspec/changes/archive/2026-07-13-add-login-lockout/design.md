## Context

`DashboardUser` (`api/src/domain.rs:154`) and `AppUser` (`api/src/domain.rs:274`) have no failure-tracking fields today. Neither login handler (`handlers/auth.rs:323`, `handlers/app_auth.rs:27`) checks anything before verifying a password. `AppState` (`state.rs`) has no in-process cache — Mongo is the only shared state across API instances, so lockout state must live on the user documents themselves, the same way the existing `removed_memberships` cooldown does.

The closest existing precedent is the membership-removal cooldown (`enforce_join_cooldown`, `auth.rs:510`) and its admin-facing `clear_cooldown` endpoint (`handlers/users.rs:158-191`) — a `RequireAdmin`-gated action that clears a time-based block by id. This design reuses that shape for the new unlock endpoints instead of inventing a new one.

Both login handlers already collapse every failure mode into one generic response (`INVALID_CREDENTIALS` for dashboard, the same principle for AppUser) specifically so a caller cannot tell "wrong password" apart from "unknown account." This design must not weaken that guarantee.

## Goals / Non-Goals

**Goals:**
- Stop unlimited password guessing against a single dashboard-user or AppUser account.
- Keep the existing non-disclosure property of both login endpoints: a locked account must be indistinguishable, from the caller's perspective, from a wrong-password attempt.
- Give an admin a way to unlock an account without touching the database directly.
- Work correctly across multiple API instances / restarts without adding an in-memory cache.

**Non-Goals:**
- Per-IP rate limiting. Left as a documented future extension (ROADMAP); this change is per-account only.
- Locking AppUsers whose Org uses `auth_source == external_db`. Their credentials are verified by the customer's own database; a local lock would only add a new external-auth failure mode to diagnose.
- Fully constant-time login responses. A timing side-channel already exists today between "unknown email" (fast path, no bcrypt) and "known email, wrong password" (slow path, bcrypt runs) — see Risks below. This change adds the locked-account branch to that same fast-path category; it does not introduce a new category of leak, and closing the pre-existing timing gap is out of scope here.
- Any new `ApiError` variant or HTTP status code for lockout — the response body and status for a locked account are identical to a wrong-password response.

## Decisions

**Schema: two plain fields on the existing user documents, not a separate collection.**
`failed_login_attempts: u32` (`#[serde(default)]`) and `locked_until: Option<bson::DateTime>` (`#[serde(default)]`) on both `DashboardUser` and `AppUser`. `#[serde(default)]` is already the codebase's convention for adding fields to existing Mongo documents (e.g. `domain.rs:34`, `:91`) — no backfill/migration script needed; existing documents deserialize with `attempts = 0`, `locked_until = None`.
- Alternative considered: a separate `login_lockouts` collection keyed by user id, mirroring `removed_memberships`. Rejected — that shape exists because a cooldown outlives the removal that caused it (the membership document is gone). Here the user document is always present, so co-locating the fields avoids an extra round-trip on every login.

**Atomicity: `find_one_and_update` with `$inc`, not read-then-write.**
Incrementing `failed_login_attempts` and conditionally setting `locked_until` must be a single atomic Mongo operation to avoid a race where two concurrent failed attempts both read `attempts = 2` and neither crosses the threshold. Implementation: `$inc: { failed_login_attempts: 1 }`, read back the new count from the update result, and issue a second update to set `locked_until` only when the new count reaches the threshold. (A single aggregation-pipeline update could do both in one round trip; plain two-step is simpler and the extra latency only occurs on the failure path, not the hot successful-login path.)

**Locked-account check happens before password verification, and skips it entirely.**
On login: look up the account → if `locked_until` is in the future, return the standard failure response immediately, without calling `password::verify`. This both avoids wasted bcrypt work and — importantly — means repeatedly hitting a locked account does not reset or extend the lock window, so an attacker cannot keep a legitimate user locked out indefinitely by hammering the endpoint after the lock is already set.

**No new error variant; locked and wrong-password responses are byte-for-byte identical.**
Reuses `ApiError::InvalidCredentials` (dashboard) and the existing generic AppUser login failure. This was a deliberate reversal from an earlier draft of this design that considered a distinct `ACCOUNT_LOCKED` error — that would have let an attacker distinguish "this account exists and is now locked" from "wrong password," which defeats part of the point of locking in the first place.

**External-auth AppUsers are exempt, checked via the Org's `auth_source` before any lockout logic runs.**
Determined by the existing per-Org `auth_source` field (`internal` vs `external_db`, see `external-db-auth` spec) — no new field needed. When `external_db`, the login handler skips lockout bookkeeping entirely and delegates straight to the existing provider dispatch.

**Config: two env vars following the `session_ttl_secs` pattern.**
`LOGIN_LOCKOUT_THRESHOLD` (u32, default `3`) and `LOGIN_LOCKOUT_DURATION_SECONDS` (u64 → `Duration`, default `3600`), parsed in `Config::from_env()` the same way `BANDAO_SESSION_TTL_SECONDS` is (`config.rs:68-74`).

**Admin unlock: one endpoint per user type, mirroring `clear_cooldown`.**
`POST /dashboard-users/{id}/unlock` and `POST /app-users/{id}/unlock`, both `RequireAdmin`, both a single update setting `failed_login_attempts = 0, locked_until = null`, both `204` on success. Kept as two endpoints (not a shared generic one) because the two user types already have entirely separate repositories, handlers, and admin-web pages — matching the rest of the codebase's existing "no premature sharing between dashboard-user and AppUser management" pattern.

**Admin-facing list responses expose a computed `is_locked: bool`, not the raw fields.**
`locked_until` in the future ⇒ `is_locked = true`. `failed_login_attempts` is never serialized to any API response — it's an internal counter, and exposing it gives an admin no actionable information beyond what `is_locked` already conveys (admin-web only needs to know whether to show the unlock button).

## Risks / Trade-offs

- **[Risk]** Timing side-channel: the locked-account fast path and the unknown-account fast path both skip bcrypt, while a real wrong-password attempt runs it — response latency alone could let a patient attacker distinguish "locked" from "wrong password" even though the JSON body is identical. → **Mitigation**: accepted as a pre-existing risk class (the unknown-account/known-account timing gap already exists in production today); no change in this design closes it. If it ever needs closing, the fix is a dummy constant-time hash comparison on every fast-path branch — noted here as a future option, not implemented now.
- **[Risk]** Legitimate users get no feedback that they're locked out — they just keep seeing "wrong password" until the hour passes or an admin unlocks them, which could generate support/admin load. → **Mitigation**: accepted trade-off in exchange for not leaking account state; admin-web's `is_locked` badge lets an admin resolve a confused user's report quickly.
- **[Risk]** Distributed guessing: an attacker who knows many usernames in one Org (e.g. via `GET /app-users`, visible to any member) can try each account up to 2 times without ever tripping a lock. → **Mitigation**: explicitly out of scope per the proposal's Non-Goals — this is exactly what a future per-IP layer is meant to catch; per-account lockout alone was the requested scope for this change.
- **[Trade-off]** Two extra Mongo round-trips on every failed login (the `$inc`, and occasionally the `locked_until` set). Negligible relative to the bcrypt cost already paid on that path.

## Migration Plan

No data migration required — `#[serde(default)]` means existing documents without the new fields deserialize as unlocked with zero attempts. Config defaults (3 attempts / 1 hour) apply immediately on deploy; no feature flag needed since the behavior is strictly additive (accounts are only ever locked after this change ships, never retroactively).
