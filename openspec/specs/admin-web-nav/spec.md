# admin-web-nav Specification

## Purpose

Defines admin-web's shared navigation shell: a single side navigation panel (Org switcher + role-computed link list) applied to every authenticated page except the pre-auth and zero-Org landing pages, replacing per-page duplicated header markup.

## Requirements

### Requirement: Authenticated pages share a single navigation layout, except pre-auth and zero-Org landing pages

The system SHALL provide a shared layout, applied to every authenticated admin-web page except `/login`, `/register`, `/no-org`, `/privacy`, and `/download` — this covers both pages that require an active Org (`/`, `/members`, `/cooldowns`, `/app-users`, `/checkin` and its sub-routes, `/settings/auth`, `/settings/api-tokens`, `/admin/join-requests`) and the org-agnostic-but-authenticated pages `/orgs/new` and `/orgs/join` (reachable without an active Org, but still meaningful to show the Org switcher on, since the caller may already belong to other Orgs). The layout consists of a persistent side navigation panel (containing the Org switcher and navigation links) and the page's own content area. No page covered by this layout SHALL duplicate the Org switcher or a "back to home" link in its own template — those affordances live in the shared layout only.

#### Scenario: Covered page renders inside the shared layout

- **WHEN** an authenticated user with an active Org navigates to `/members`
- **THEN** the side navigation panel (Org switcher + nav links) is visible
- **AND** the page's own content renders inside the layout's content area, without its own duplicate header/Org-switcher markup

#### Scenario: Org-agnostic-but-authenticated pages still use the shared layout

- **WHEN** an authenticated user with no active Org (or with other existing Org memberships) navigates to `/orgs/new` or `/orgs/join`
- **THEN** the side navigation panel is visible, including the Org switcher

#### Scenario: Pre-auth and zero-Org landing pages do not use the shared layout

- **WHEN** an authenticated user with no active Org is on `/no-org`, or any user is on `/login` or `/register`
- **THEN** the shared side navigation panel is not rendered

### Requirement: Navigation links are grouped by relationship, determined by role, from a single source

The system SHALL compute the navigation panel's contents from the caller's role in `current_org`, in one place shared by all covered pages (not duplicated per-page). The panel SHALL be structured, top to bottom, as: `打卡看板` (top-level, no children); `成員管理` (top-level, links to `/members`) with `加入申請` (links to `/admin/join-requests`) as its child, visible only to `admin`; `App 使用者` (top-level, links to `/app-users`) with `驗證來源` (links to `/settings/auth`) as its child, visible only to `admin`; a `進階工具` group label — not itself a link, since its two children are equally-weighted siblings with no single "primary" destination — visible only to `admin`, containing `API Token` (links to `/settings/api-tokens`) and `冷卻管理` (links to `/cooldowns`) as its children; and `下載 App` (top-level, no children) last. Child items SHALL always render expanded directly beneath their parent — the panel SHALL NOT implement collapse/expand (accordion) interaction for any group. Both `admin` and `member` SHALL see `打卡看板`, `成員管理`, `App 使用者`, and `下載 App`. Only `admin` SHALL additionally see `加入申請`, `驗證來源`, and the `進階工具` group (with its two children).

#### Scenario: Admin sees the full nested navigation, in order

- **WHEN** a dashboard `admin` views the side navigation panel
- **THEN** it lists, top to bottom: 打卡看板, 成員管理 (with child 加入申請), App 使用者 (with child 驗證來源), 進階工具 (with children API Token and 冷卻管理), 下載 App

#### Scenario: Parent items with children remain independently navigable

- **WHEN** an admin clicks the `成員管理` label itself (not its `加入申請` child)
- **THEN** the browser navigates to `/members`
- **AND** `加入申請` remains visible as its child, unaffected

#### Scenario: The 進階工具 group label is not itself a link

- **WHEN** an admin views the `進階工具` entry in the navigation panel
- **THEN** it renders as non-interactive text, not a clickable link
- **AND** its two children (`API Token`, `冷卻管理`) are each independently clickable links

#### Scenario: Member sees a flat reduced navigation; parents with no visible children render as plain links

- **WHEN** a dashboard `member` views the side navigation panel
- **THEN** it includes `打卡看板`, `成員管理`, `App 使用者`, and `下載 App`, in that order
- **AND** `成員管理` and `App 使用者` render as plain links with no visible child items, since their only children are admin-only
- **AND** no `進階工具` label or its children appear anywhere in the panel

### Requirement: Pending join-request count is visible from the navigation

The system SHALL display the count of pending join requests for `current_org` as a badge on the 加入申請 navigation link, refreshed on the same polling cadence as before this change (30 seconds). This badge SHALL only be visible to `admin` (join requests remain an admin-only capability).

#### Scenario: Pending count badge shown to admin

- **WHEN** `current_org` has 3 pending join requests and a dashboard admin views the navigation panel
- **THEN** the 加入申請 link shows a badge with the value 3

### Requirement: Navigation panel is usable on narrow viewports

The system SHALL render the navigation panel as persistently visible on wide viewports and as a collapsible panel (hidden by default, toggled via a visible control) on narrow viewports. Collapsing or expanding the panel SHALL NOT navigate away from the current page or lose in-progress form state on that page.

#### Scenario: Narrow viewport hides the panel by default

- **WHEN** a covered page is viewed on a narrow viewport
- **THEN** the navigation panel is collapsed by default
- **AND** a visible control is present to expand it

#### Scenario: Expanding the panel does not affect page state

- **WHEN** the user expands the navigation panel while filling in a form on the current page
- **THEN** the form's entered values are unchanged after the panel is expanded or collapsed again

### Requirement: Org switcher popup fits within the navigation panel width

The Org switcher's popup (listing the caller's Org memberships, grouped by "我擁有的" / "我加入的") SHALL render fully within the horizontal bounds of the navigation panel, regardless of viewport width. The popup SHALL NOT extend past the navigation panel's left or right edges such that any part of its content (an Org's name or role badge) becomes inaccessible or invisible within the viewport.

#### Scenario: Popup does not clip off-screen in the narrow, always-visible sidebar

- **WHEN** an admin with a current Org opens the Org switcher popup inside the persistent navigation panel
- **THEN** every row's Org name and role badge are fully visible within the viewport
- **AND** no part of the popup renders outside the navigation panel's horizontal bounds
