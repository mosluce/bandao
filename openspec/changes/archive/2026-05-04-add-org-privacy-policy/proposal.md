## Why

The upcoming `add-location-tracking-app` change needs a worker-facing
"組織政策" link on its consent dialog so workers can read what data is being
collected before they tap `[同意並上班]`. That link has nowhere to go yet.

Beyond the immediate dependency, Argus has been collecting personal data
(account, password hash, checkin events with coordinates) since
`add-app-shell` shipped, and currently has no public-facing privacy policy
at all. Per Taiwan's 個資法 §8, data subjects have a right to know what is
being collected, why, by whom, and for how long, before collection happens.
Shipping a privacy policy page is overdue independent of the location
tracking dependency.

This change adds a single public page on `admin-web` at `/privacy` covering
the platform-level privacy stance: what we collect, why, how long we keep
it, who can access it, and how data subjects exercise their rights. The
content is platform-uniform (one page, all Orgs) — not per-Org — to keep
the MVP small. Per-Org override is deferred to a future change if the
need arises.

## What Changes

- **New public page at `/privacy`** in `admin-web`, no auth required, no
  middleware applied.
- **9 sections** covering: scope, data collected, collection purpose,
  retention periods, who has access, data subject rights (個資法 §3 / §10
  / §11), cookies / sessions, contact, policy update notice.
- **Forward-looking content** — explicitly mentions location tracking
  (90-day retention, only when the Org enables it) and the 5-year retention
  for checkin events (per 勞基法 §30 V), even though location tracking
  isn't shipped yet, so the policy doesn't need a same-day update when
  `add-location-tracking-server` lands.
- **Disclaimer footer**: `本政策範本未經法律審查，建議您所屬組織自行確認符合
  當地法規。`
- **Contact placeholder**: `noreply@example.com` — operator replaces with
  a real address before public launch.
- **Last-updated date** rendered from a constant in the page source, not
  the build time, so the date only changes when the policy actually
  changes (not on every redeploy).
- **No API changes**, no DB changes, no `app/` changes. Purely a static
  Nuxt page.

Out of scope (deferred / future ROADMAP):

- Per-Org privacy policy override (each Org editing their own).
- Worker-facing "我的資料" page (data subject access exercise) inside the
  Flutter app.
- Multi-language privacy policy (zh-TW only for v1).
- Legal review by counsel — explicitly noted in the disclaimer.
- Cookie consent banner (zero tracking cookies; existing session cookie
  is functional/necessary, doesn't require consent under most readings of
  current TW law).

## Capabilities

### New Capabilities

- `org-privacy-policy`: public-facing privacy policy page at `/privacy`,
  platform-uniform content covering data collection, retention, access,
  and data subject rights.

### Modified Capabilities

(none — no existing spec's requirements change.)

## Impact

- **Code**: one new Vue page at `admin-web/pages/privacy.vue` plus a
  constant for `LAST_UPDATED_AT`. No new dependencies.
- **Routing**: `/privacy` added to the public route surface.
  `middleware/auth.ts` is not applied; the page is accessible without
  login. The `guest` middleware is also NOT applied — authenticated users
  can also view the page.
- **No API / DB / app changes.**
- **Tests**: one Nuxt component test verifying the page renders all 9
  sections + the disclaimer + the placeholder contact.
- **ROADMAP**: this change's tasks include a `ROADMAP.md` cleanup pass —
  removing the archived `add-app-checkin` mention and replacing the single
  `add-location-tracking` line with three properly-scoped items
  (`add-location-tracking-server`, `-app`, `-dashboard`) carrying the
  decisions locked during the explore session.
- **Downstream**: unblocks the `add-location-tracking-app` consent UI work
  (it can link to `<admin-web>/privacy`).
