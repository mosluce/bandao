## MODIFIED Requirements

### Requirement: AppUser identity is per-Org and admin-managed

The system SHALL maintain a `app_users` collection holding identity records for end-users of the mobile app. Each row SHALL contain `_id`, `org_id` (FK to `Org`, immutable), `username`, `username_lower` (lowercased copy used for case-insensitive uniqueness), `display_name`, `password_hash`, `status: active | disabled`, `needs_password_change: bool`, `last_login_at: DateTime | null`, `created_by_dashboard_user_id`, `created_at`, `updated_at`. The system SHALL enforce a unique index on `(org_id, username_lower)`. The system SHALL NOT provide any self-registration path for AppUsers; identities are created exclusively through admin endpoints.

Each row SHALL additionally hold `legacy_backfill_done_at: DateTime | null`, a one-shot marker set once a legacy check-in backfill has completed successfully for this AppUser (see `legacy-checkin-backfill`). It is absent/`null` on all AppUsers by default, including those in Orgs with no `legacy_backfill` configuration, for whom it stays permanently `null`.

#### Scenario: New AppUser row records its creator and Org

- **WHEN** an admin successfully creates an AppUser
- **THEN** the new row's `org_id` equals the admin's `current_org_id`
- **AND** `created_by_dashboard_user_id` equals the admin's user id
- **AND** `status = active`, `needs_password_change = true`, `last_login_at = null`
- **AND** `legacy_backfill_done_at = null`

#### Scenario: Same username can exist in different Orgs

- **WHEN** Org A already has an AppUser with `username_lower = "alice"`
- **AND** an admin in Org B creates an AppUser with `username = "Alice"`
- **THEN** the request succeeds — the unique index is scoped to `(org_id, username_lower)`

#### Scenario: Username is case-insensitive within an Org

- **WHEN** an admin in Org A attempts to create a second AppUser with `username = "ALICE"` while one with `username_lower = "alice"` already exists in Org A
- **THEN** the request is rejected with `USERNAME_TAKEN`

#### Scenario: legacy_backfill_done_at stays null without a configured source

- **WHEN** an AppUser belongs to an Org with no `legacy_backfill` configuration
- **THEN** `legacy_backfill_done_at` remains `null` regardless of how many times the AppUser logs in
