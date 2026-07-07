# web-download-page Specification

## Purpose
TBD - created by syncing change add-web-download-page. Update Purpose after archive.
## Requirements
### Requirement: Public download page reachable without authentication

The admin-web SHALL serve a `/download` page that renders for unauthenticated visitors without redirecting to login, following the same public-route convention as the existing privacy page.

#### Scenario: Logged-out visitor opens the shared link

- **WHEN** a visitor with no active session navigates directly to `/download`
- **THEN** the download page renders fully
- **AND** the visitor is NOT redirected to the login page

#### Scenario: Logged-in admin opens the page

- **WHEN** an authenticated admin navigates to `/download`
- **THEN** the download page renders the same content as for a logged-out visitor

### Requirement: Both store download points are presented as official badges

The download page SHALL present an App Store download point and a Google Play download point, each rendered as that vendor's official store badge, using locally-served badge assets.

#### Scenario: App Store badge links to the unlisted iOS build

- **WHEN** the download page renders
- **THEN** an official "Download on the App Store" badge is shown
- **AND** it links to `https://apps.apple.com/app/id6767153656`

#### Scenario: Google Play badge links to the public Android listing

- **WHEN** the download page renders
- **THEN** an official "Get it on Google Play" badge is shown
- **AND** it links to `https://play.google.com/store/apps/details?id=tw.ccmos.app.bandao`

### Requirement: Each store link has a scannable QR code

The download page SHALL display a QR code for each store link, generated client-side from the same link constants, so the QR always matches the link it accompanies.

#### Scenario: QR encodes the App Store link

- **WHEN** the download page renders
- **THEN** a QR code is shown next to the App Store badge
- **AND** scanning it resolves to `https://apps.apple.com/app/id6767153656`

#### Scenario: QR encodes the Google Play link

- **WHEN** the download page renders
- **THEN** a QR code is shown next to the Google Play badge
- **AND** scanning it resolves to `https://play.google.com/store/apps/details?id=tw.ccmos.app.bandao`

### Requirement: Page links to privacy policy and support contact

The download page SHALL link to the privacy policy and expose the support email so visitors have a path to policy and help without an account.

#### Scenario: Privacy and support links present

- **WHEN** the download page renders
- **THEN** a link to `/privacy` is shown
- **AND** the support email `support@ccmos.tw` is shown as a `mailto:` link

### Requirement: Admin home links to the download page

The authenticated admin home SHALL include a "下載 App" navigation link, in the "管理員工具" card, pointing to `/download`.

#### Scenario: Admin sees the download menu entry

- **WHEN** an authenticated admin views the home page
- **THEN** a "下載 App" link is shown in the "管理員工具" card
- **AND** clicking it navigates to `/download`

#### Scenario: Menu entry is not required for unauthenticated access

- **WHEN** an unauthenticated visitor cannot see the admin home menu
- **THEN** they can still reach `/download` directly via its URL
