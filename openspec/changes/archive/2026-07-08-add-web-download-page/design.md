## Context

admin-web is a Nuxt 3 SPA/SSR (pnpm, Tailwind) served at `bandao-admin.ccmos.tw`. It has no shared layout or nav component — the "menu" is a set of `NuxtLink`s inside the "管理員工具" card on `pages/index.vue`, which is auth-gated (`definePageMeta({ middleware: 'auth' })`). `pages/privacy.vue` is the established pattern for a **public** page: it applies no middleware, so it renders for authenticated users, unauthenticated visitors, and external webview/browser launches alike.

班到 iOS is under Unlisted Distribution (unsearchable; direct link only — see `mobile-release` / the unlisted-distribution decision), so the download page is effectively iOS's sole distribution surface, not just marketing. Android is a normal public Play listing.

## Goals / Non-Goals

**Goals:**
- A public `/download` page reachable without login, so an admin can share the raw URL with staff who have no admin account.
- Both store download points present as official badges, each with a scannable QR code for in-person onboarding.
- Discoverable by admins from the authenticated home menu.
- Reuse existing patterns (public-route convention, Tailwind styling) — no new architecture.

**Non-Goals:**
- User-Agent platform detection / auto-emphasis of the matching store (follow-up).
- A standalone marketing landing site or moving `/privacy` to `bandao.ccmos.tw` (separate ROADMAP `[cross]` item).
- Any API, auth, or data-model change.

## Decisions

**1. Page is public (no middleware), menu link lives behind auth.**
The audience that needs to *download* is staff (app users), who typically lack an admin login; the audience that *discovers the menu* is the admin. Resolve the mismatch by making the page public (mirroring `privacy.vue`) while placing the menu `NuxtLink` in the auth-gated home card. The admin can either show the page themselves or paste `…/download` into LINE/email for staff.
*Alternative considered:* auth-gate the page too — rejected, it would bounce the very users it targets to a login wall.

**2. Country-neutral iOS link `apps.apple.com/app/id6767153656`.**
The app is Taiwan-only under unlisted distribution; a region-specific `/us/` path can fail to resolve. The id-only, country-neutral form lets Apple route to the viewer's eligible region.
*Alternative considered:* `/tw/…` — works today but hard-codes region; country-neutral is more robust if availability expands.

**3. QR codes generated client-side via a JS QR library.**
Keeps the QR in lockstep with the link constants — change a URL and the QR regenerates on render, no committed image to re-export. Rendered to inline SVG/canvas at build/runtime.
*Alternative considered:* commit static PNG/SVG QR images — rejected, they drift from the source-of-truth URLs and add a manual regen step. Pick a small, dependency-light, maintained library (e.g. `qrcode`).

**4. Official store badges as the buttons.**
Apple and Google both require their official badge artwork and forbid custom-styled equivalents in most contexts. Use the official "Download on the App Store" and "Get it on Google Play" badge assets, sized per each vendor's minimum-clear-space rules. Store the assets under admin-web's public assets so they're served locally (no third-party badge hotlinking).

**5. Store link/id constants centralized in the page (or a small shared const).**
iOS id `6767153656`, Android package `tw.ccmos.app.bandao`. Keep them as named constants so the marketing_url follow-up and any future reuse reference one place.

## Risks / Trade-offs

- **[iOS country-neutral link may still land unavailable-region visitors on an error]** → App is TW-only today; acceptable for the current audience. Revisit if regions expand (memory: region availability is freely editable in App Store Connect).
- **[Store badge brand-guideline violations can draw vendor complaints]** → Use official assets at approved sizes/clear-space; don't recolor or alter.
- **[QR library adds a client dependency to admin-web]** → Choose a small, well-maintained lib; render only on `/download` so it doesn't weigh the rest of the app.
- **[marketing_url.txt update depends on the page being live first]** → Sequenced as a post-deploy step; the page URL must exist before App Store Connect can accept it.

## Migration Plan

1. Ship `/download` + the home menu link; deploy admin-web to `bandao-admin.ccmos.tw`.
2. Verify the page renders logged-out (incognito) and the badges/QRs resolve to the correct store pages.
3. Update `app/store_metadata/ios/marketing_url.txt` to the live `…/download` URL; push to App Store Connect on the next metadata sync.
Rollback: remove the menu link and page; blank marketing_url again. No data migration, so rollback is a plain revert.

## Open Questions

- Which QR library exactly (bundle size vs. API)? Resolve at implementation; `qrcode` is the default candidate.
- Do we also want the Android link to carry a `referrer`/UTM param for install attribution? Deferred unless attribution is needed.
