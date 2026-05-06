# org-privacy-policy Specification

## Purpose
TBD - created by archiving change add-org-privacy-policy. Update Purpose after archive.
## Requirements
### Requirement: Privacy policy page is publicly accessible at `/privacy`

The system SHALL provide a public-facing privacy policy page at the URL
path `/privacy` on the `admin-web` deployment. The page SHALL be reachable
without authentication: the request SHALL succeed for users without a
session cookie, with an expired session cookie, or with a valid session
cookie. The system SHALL NOT redirect any class of caller away from
`/privacy` based on auth state.

#### Scenario: Unauthenticated visitor reaches the page

- **WHEN** a visitor with no session cookie navigates to `/privacy`
- **THEN** the page renders successfully (HTTP 200)
- **AND** no redirect to `/login` occurs

#### Scenario: Authenticated visitor reaches the page

- **WHEN** a visitor with a valid bandao session cookie navigates to `/privacy`
- **THEN** the page renders successfully
- **AND** no redirect away from `/privacy` occurs

#### Scenario: Reachable from external webview / browser launch

- **WHEN** the URL `https://<admin-web-host>/privacy` is opened in an
  external browser (e.g. iOS Safari, Android Chrome) without any cookies
- **THEN** the page renders successfully
- **AND** no API call, login redirect, or auth handshake is required

### Requirement: Privacy policy page covers nine substantive sections

The page SHALL render nine sections in the following order, each as a
heading-and-body pair, covering at minimum the listed subject matter:

1. **適用範圍** — scope: bandao platform plus the user's Org services.
2. **我們蒐集的資料** — data collected: account credentials, checkin
   events with coordinates, location tracking pings (conditional on the
   user's Org enabling that feature).
3. **蒐集目的** — purpose: 勞基法 §30 obligation, work-site verification,
   safety, dispute evidence.
4. **保留期** — retention: checkin events 5 years (per 勞基法 §30 V),
   location tracking pings 90 days, account identity for the lifetime of
   the account.
5. **誰能存取** — access: Org admin/owner, bandao platform operations.
6. **您的權利** — data subject rights: 個資法 §3 §10 §11 (access,
   correction, deletion, withdrawal of consent); exercise via the user's
   Org admin.
7. **Cookie / Session** — strictly necessary session cookie only; no
   tracking or advertising cookies.
8. **聯絡方式** — contact: Org owner for Org-specific concerns; placeholder
   `noreply@example.com` for platform contact.
9. **政策更新** — update notice: 30-day advance notice for material
   changes; last updated date.

#### Scenario: All nine sections are present

- **WHEN** the page is rendered
- **THEN** each of the nine section headings appears in the listed order

#### Scenario: Location tracking is mentioned conditionally

- **WHEN** section 2 (我們蒐集的資料) is read
- **THEN** location tracking is described as collected only when the user's
  Org has enabled it (e.g. phrased as "若您所屬組織開啟此功能時")
- **AND** the section does NOT imply that location tracking is currently
  active for all users

#### Scenario: Retention periods are stated explicitly

- **WHEN** section 4 (保留期) is read
- **THEN** the page states the 5-year retention for checkin events with a
  reference to 勞基法 §30 V
- **AND** states the 90-day retention for location tracking pings
- **AND** states that account identity persists for the lifetime of the
  account

### Requirement: Privacy policy renders a stable last-updated date

The page SHALL render a "最後更新日期：YYYY-MM-DD" line in section 9 (or
adjacent to it). The date SHALL be sourced from a source-code constant
that is updated only when the policy text changes — NOT from build
timestamp, deploy time, or current time. Successive deploys without
content changes SHALL show the same date.

#### Scenario: Date is shown in section 9

- **WHEN** the page is rendered
- **THEN** the text "最後更新日期：" followed by an ISO-format date
  (YYYY-MM-DD) appears in or adjacent to section 9

#### Scenario: Date does not change on rebuild

- **WHEN** the page is rebuilt and redeployed without any source-code
  edit to the policy text or the date constant
- **THEN** the rendered date is identical before and after the rebuild

### Requirement: Privacy policy displays a non-legal-review disclaimer

The page SHALL render the disclaimer "本政策範本未經法律審查，建議您所屬組織
自行確認符合當地法規。" (or substantially equivalent text) in a footer-style
position below the main content sections.

#### Scenario: Disclaimer is visible in the footer

- **WHEN** the page is rendered
- **THEN** the disclaimer text "本政策範本未經法律審查" appears
- **AND** it sits below section 9 (政策更新)

### Requirement: Privacy policy applies neither auth nor guest middleware

The Vue page file SHALL NOT call `definePageMeta` with `middleware: 'auth'`
or `middleware: 'guest'`. The route SHALL be reachable from both
authenticated and unauthenticated states without redirect.

#### Scenario: Page source contains no middleware directive

- **WHEN** the page source `admin-web/pages/privacy.vue` is inspected
- **THEN** there is no `middleware: 'auth'` or `middleware: 'guest'`
  directive in any `definePageMeta` call

#### Scenario: Authenticated user is not redirected to home

- **WHEN** a logged-in user with a valid `current_org` navigates to
  `/privacy`
- **THEN** the URL stays at `/privacy`
- **AND** the page renders the privacy policy content

### Requirement: Privacy policy uses placeholder contact, not a real address

The page SHALL render `noreply@example.com` in section 8 as the platform
contact placeholder. The text SHALL make it visually clear that this is a
placeholder pending operator replacement (e.g. through phrasing or
adjacent disclaimer copy). The placeholder SHALL be replaced with a real
address by the operator before any production / public launch — but that
replacement is operator responsibility, not part of this change's
acceptance criteria.

#### Scenario: Placeholder address is rendered

- **WHEN** section 8 (聯絡方式) is read
- **THEN** `noreply@example.com` appears as the bandao platform contact
- **AND** the surrounding copy or context makes it visually clear that
  this is a placeholder
