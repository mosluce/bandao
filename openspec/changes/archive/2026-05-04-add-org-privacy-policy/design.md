## Context

Argus has been collecting personal data since `add-app-shell` and is about
to start collecting much more sensitive data (continuous location pings)
in the next ROADMAP iteration. Workers tapping `[同意並上班]` on the
forthcoming consent dialog need a real URL to read before they consent.
We also have no public privacy policy of any kind — overdue under 個資法
§8.

The decision to make this its own change (rather than bundling into the
location-tracking-server change that creates the consent requirement)
came from the explore session: privacy policy is a small, self-contained,
shippable artifact that unblocks at least three other changes
(`add-location-tracking-server` / `-app` / `-dashboard`) and has standalone
value.

## Goals / Non-Goals

**Goals:**

- Ship a single public `/privacy` page in `admin-web` that covers the
  platform-level privacy stance accurately enough to support the location
  tracking consent dialog, and accurately enough that a non-lawyer would
  recognise it as a real privacy policy (vs Lorem ipsum).
- Make the policy forward-looking — mention location tracking + 90-day
  retention now, so when `add-location-tracking-server` lands the policy
  doesn't need a same-day amendment.
- Keep the page maintainable: future updates are a one-line `LAST_UPDATED_AT`
  bump + content edits in a single Vue file.
- Make the page reachable without authentication, from any device,
  including the Flutter app's webview / external browser launch.

**Non-Goals:**

- Per-Org privacy policy override. One platform-wide page; future change
  upgrades to per-Org if needed.
- Legal review. Explicit disclaimer in the page footer says the content
  is not lawyer-reviewed.
- Internationalization. zh-TW only.
- Cookie consent banner. We don't use tracking cookies; the session
  cookie is functional/necessary and doesn't require explicit consent
  under the current reading of TW law.
- Worker-facing data subject access tooling (no "我的資料" page inside the
  Flutter app). Workers exercise their rights by contacting their Org
  admin, as documented in the policy.
- A separate Terms of Service. The policy covers privacy only.

## Decisions

### Platform-uniform, not per-Org

We considered three approaches:

| approach | URL pattern | content storage | who edits | MVP cost |
|---|---|---|---|---|
| A. Platform uniform | `/privacy` | Vue page constant | dev (PR) | small |
| B. Per-Org override | `/orgs/<slug>/privacy` | DB + admin UI | Org admin | medium |
| C. Hybrid (platform + Org appendix) | both URLs | Vue + DB | both | large |

We pick **A**. Rationale:

- Argus is a B2B SaaS where the **platform** does the data collection
  (servers, DB, infra) — Orgs are tenants on the platform. The privacy
  policy is fundamentally about what the *platform* does with data;
  Org-specific policies would be additional disclosures, not replacements.
- B/C add an admin-web editor surface (rich text editor, sanitization,
  versioning, "last edited by") that's out of proportion for MVP.
- Real-world precedent: most B2B SaaS run a single privacy policy URL
  (Slack, Notion, Stripe) regardless of tenant — Orgs that need extra
  disclosures usually link a third-party document.
- Upgrade path is clean: B becomes a future change if the need arises;
  the URL `/privacy` remains the platform fallback.

### Content is hard-coded in a Vue page, not loaded from DB

The policy text is structured zh-TW prose. Storing it in MongoDB and
rendering via SSR would let admin-web edit without a deploy, but:

- Single-author content (only "Argus the platform" speaks here, not
  individual Orgs) doesn't need a CMS.
- Versioning + diff review of policy text is exactly what `git` does well.
- Markdown / rich text in DB requires sanitization; static Vue text doesn't.
- Edit cadence in practice is "rarely" — bumping version + content via
  PR is appropriate.

### Forward-looking content includes location tracking

The policy mentions location tracking + 90-day retention now, even though
the feature isn't shipped. Reasoning:

- Saves us from a same-day policy amendment when
  `add-location-tracking-server` lands and Orgs start enabling it.
- Adds a phrase like "若您所屬組織開啟此功能" so it's accurate today
  (no Org has the feature) and stays accurate after launch.
- 個資法 §8 requires disclosure *before* collection; having the policy
  ready is a precondition, not a post-hoc fix.

### `LAST_UPDATED_AT` is a constant, not the build time

Rendering `new Date(__BUILD_TIMESTAMP__).toLocaleDateString('zh-TW')` is
tempting but wrong: the policy date should reflect when **the policy
content** changed, not when the bundle was built. A constant in the page
file (e.g. `const LAST_UPDATED_AT = '2026-05-04'`) bumped only when the
content changes is the cleanest signal.

### No middleware applied

The page sits at `admin-web/pages/privacy.vue` with no `definePageMeta({
middleware })`. Both `auth` and `guest` middlewares apply only to pages
that opt in. Login-screen is `guest` (redirect away if logged in); home
is `auth` (redirect to /login if not logged in); privacy is **neither** —
both authenticated and unauthenticated users can read it.

### Accessible from Flutter app webview / browser launch

The Flutter consent dialog will use `url_launcher` (or in-app webview)
pointing at `<admin-web base URL>/privacy`. Because the page applies no
middleware, no cookie state, no API call, the iOS / Android webview
opens it cleanly without any auth handshake.

The base URL is configurable per environment in the Flutter app (we
already have the `dev.api_base_url_override` mechanism); admin-web URL
doesn't need a parallel override since it's only navigated to externally.
For dev, hard-code the dev admin-web URL (`http://localhost:3000`); for
prod, the deployed admin-web URL. We'll address the cross-app URL config
in `add-location-tracking-app`, not here.

### Section structure

Nine sections, in this order, designed to flow as a single read:

```
1. 適用範圍
   - Argus 平台 + 您所屬組織的服務
2. 我們蒐集的資料
   - 帳號識別 (email / username, password hash)
   - 出勤事件 (clock_in/out/transfer + 座標 + 反向地理編碼)
   - 位置軌跡 (若您所屬組織開啟此功能時)
3. 蒐集目的
   - 勞基法 §30 出勤紀錄義務
   - 工作現場確認、安全、糾紛佐證
4. 保留期
   - 出勤事件：5 年 (依勞基法 §30 V)
   - 位置軌跡：90 天
   - 帳號識別：帳號有效期間
5. 誰能存取
   - 您所屬組織的 admin / owner
   - Argus 平台維運人員 (系統管理需要)
6. 您的權利
   - 個資法 §3 §10 §11：查閱、更正、刪除、撤回同意
   - 行使方式：聯絡您的組織 admin
7. Cookie / Session
   - 必要 cookies (登入狀態)
   - 不使用追蹤 / 廣告 cookies
8. 聯絡方式
   - 各組織責任歸屬 Org owner
   - Argus 平台聯絡：noreply@example.com (placeholder)
9. 政策更新
   - 重大更新 30 天前通知
   - 最後更新日期：YYYY-MM-DD (rendered from LAST_UPDATED_AT)
```

The disclaimer footer below section 9:

> 本政策範本未經法律審查，建議您所屬組織自行確認符合當地法規。

## Risks / Trade-offs

- **Policy is not lawyer-reviewed.** Mitigation: explicit disclaimer.
  Real launch should commission a review. We document this expectation in
  the policy itself + in the change's task list (final smoke task).
- **Per-Org tenants may find platform-uniform policy insufficient.**
  Mitigation: a future change adds Org override, and the platform policy
  remains the fallback. The current policy includes language that points
  Orgs to "your Org admin" for Org-specific questions, which buys time.
- **Policy updates require a deploy.** Mitigation: this is intentional —
  policy edits go through PR review, which is appropriate for legal
  text. Edit cadence is low.
- **Forward-looking mention of location tracking before it ships could
  confuse readers** ("the policy says you collect location, but you
  haven't asked me yet"). Mitigation: phrase as conditional ("若您所屬
  組織開啟此功能時") so it's accurate today (no Org has it) and stays
  accurate post-launch.
- **Placeholder email may stay in production by accident.** Mitigation:
  the placeholder uses `example.com` which any reviewer will recognise as
  fake; the change's task list calls out replacing it before the production
  cutover. The release checklist (separate from this change) should also
  include "verify privacy policy contact email" line.
- **Cross-app URL coupling.** The Flutter app will need to know the
  admin-web URL. We defer that wiring to `add-location-tracking-app`. For
  this change we only ship the page; no Flutter coordination needed yet.

## Migration Plan

No data migration. No backwards compatibility concerns. New URL, no
existing URL changed.

For developers:

1. Pull, run `pnpm install` (no new packages).
2. `pnpm dev` — page is at `http://localhost:3000/privacy`.

For end-users:

- Page becomes accessible the moment the change deploys. No user-facing
  notification needed (it's purely additive — there was no privacy policy
  before).

For operators:

- Replace `noreply@example.com` with a real contact address before any
  external launch / public link from app.

## Open Questions

- **Should the `last updated date` be visible at the top of the page or
  only at the bottom?** I lean bottom (next to section 9), matching the
  flow of the policy. Top would be more SEO-friendly but the page isn't
  meant to be SEO-indexed at this stage.
- **Should we add a `/terms` placeholder alongside?** Out of scope — but
  if the operator wants a "Terms of Service" page later, the same Vue
  page pattern applies.
- **Should the page block search engine indexing?** Probably not — a
  publicly-accessible privacy policy is appropriate to be indexed. We
  don't add a `noindex` meta tag.
