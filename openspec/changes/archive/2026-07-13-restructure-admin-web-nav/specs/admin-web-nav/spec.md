## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Org switcher popup fits within the navigation panel width

The Org switcher's popup (listing the caller's Org memberships, grouped by "我擁有的" / "我加入的") SHALL render fully within the horizontal bounds of the navigation panel, regardless of viewport width. The popup SHALL NOT extend past the navigation panel's left or right edges such that any part of its content (an Org's name or role badge) becomes inaccessible or invisible within the viewport.

#### Scenario: Popup does not clip off-screen in the narrow, always-visible sidebar

- **WHEN** an admin with a current Org opens the Org switcher popup inside the persistent navigation panel
- **THEN** every row's Org name and role badge are fully visible within the viewport
- **AND** no part of the popup renders outside the navigation panel's horizontal bounds
