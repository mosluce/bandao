## Why

`add-app-checkin` shipped the full mobile checkin client and survived live smoke against the existing `/app/checkin/*` surface. Smoke surfaced four follow-up issues that were triaged out of the main change to keep it archive-able:

1. When a queued event syncs successfully, it visibly **disappears** from `/history` until the next page is loaded — the local `pending_events` row is deleted on 2xx, but the server-fetched events list in the `HistoryScreen` state hasn't ingested the new event.
2. When the org admin flips `transfer_enabled = false`, the app keeps rendering `[轉出]` / `[轉入]` buttons — users only discover the disabled state by submitting and getting a `failed` row back.
3. The home screen logout menu fires immediately, even when the queue still holds events that will be wiped on a different-user login. A user sharing a device with a coworker can lose data without warning.
4. (Minor) Two pieces of staleness — `auth.org` and `checkin_status` are fetched at login and never refreshed, so admin-side flips and force-checkouts only reflect after a cold start.

Plus the `history_screen` widget test deferred from `add-app-checkin` (item 14.8) — easy to fold in alongside the history-merge changes from item 1.

## What Changes

- **Just-synced events stay visible in `/history`.** A new `recentlySyncedEventsProvider` (Riverpod `StateProvider<List<CheckinEventDto>>`) is fed by the queue processor on every successful submit (existing `SubmitCheckinEventResponse.event` payload). `HistoryScreen` merges three sources: local queue rows, server-fetched events, recently-synced events. Sort remains `occurred_at_client` desc. Dedupe by event `id` when a paginated server fetch later includes a recently-synced event. No animation — straight `ListView` swap.

- **`HomeButtons` honors `Org.checkin.transferEnabled`.** When the cached value is `false`, the on_site button set collapses to `[下班]` only (drops `[轉出]`); the in_transit set also collapses to `[下班]` only (drops `[轉入]`). off_duty is unaffected. This trusts the cache; staleness is closed below.

- **Logout confirms when queue is non-empty.** The home `…` menu's `登出` action checks the queue stream for the current user and counts rows in any of `pending` / `sending` / `failed`. If count > 0, surface a `showDialog` with copy `你還有 N 筆事件未處理。登出後若由其他帳號登入，這些事件會被清除。確定要登出？` and `[取消] [仍要登出]` actions. Cancel returns to home; confirm proceeds with `authProvider.logout()`. Empty queue → existing immediate logout (no extra tap).

- **App resume refreshes `/me` and `/app/checkin/status`.** The existing `WidgetsBindingObserver.didChangeAppLifecycleState` hook in `home_screen.dart` (currently only re-checks location permission) gains two more refreshes: `authProvider.refreshMe()` and `checkinStatusProvider.notifier.refresh()`. This closes the staleness window for `transfer_enabled` flips and any admin-side force-checkout.

- **History pull-to-refresh.** `HistoryScreen` wraps its `ListView.separated` in a `RefreshIndicator`. The pull triggers: clear the recently-synced cache, refetch the first server page (resets `_serverEvents` + `_hasMore`), and refresh `checkinStatusProvider`. Local queue stream is already live so no explicit refresh needed.

- **History widget test.** `test/features/checkin/presentation/history_screen_test.dart` covering: pending+synced merge, just-synced row appearing in place after the recently-synced provider emits, failed-row dismiss flow, `[載入更多]` triggers a paginated fetch, pull-to-refresh resets the page state.

- **Localization additions.** New strings: logout-confirm dialog title / body / buttons, no other new copy needed (button hide reuses existing labels; pull-to-refresh uses platform default).

Out of scope:

- AnimatedList transitions for the just-synced row swap (deferred — straight swap is acceptable for v1).
- App resume background-fetch coalescing (avoid hammering `/me` if the user backgrounds-and-foregrounds rapidly). If observed in practice, separate fix.
- Hiding `[轉出]` / `[轉入]` on the **server** if `transfer_enabled = false` — server already returns `TRANSFER_DISABLED` on the request, the change here is purely a client UX guard.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `app-checkin`: 
  - **MODIFIED** *Home action buttons follow the active checkin status* — `[轉出]` / `[轉入]` SHALL be hidden when `Org.checkin.transferEnabled = false`.
  - **MODIFIED** *History merges server events with local queue rows* — the merge gains a third source: a recently-synced events cache populated by the queue processor's success path. Dedupe by event id when a server fetch returns an event already in the cache.
  - **ADDED** *Logout requires confirmation when the queue holds non-empty rows for the current user*.
  - **ADDED** *History pull-to-refresh refetches the first server page and resets the recently-synced cache*.
  - **ADDED** *App resume refreshes `/me` and the checkin status* (closes the org-settings + status staleness window).

## Impact

- **Code**: changes confined to `app/lib/features/checkin/` (presentation + state providers + queue processor) and `app/lib/features/auth/presentation/home_screen.dart` (logout confirm + resume hook). One new state provider (`recentlySyncedEventsProvider`). One new dialog widget. One small public method on `AuthNotifier` (`refreshMe()`). One additional method on `CheckinStatusNotifier` (already has `refresh()`).
- **No API changes**: this change is pure mobile-client polish on top of the existing `/app/*` surface. No Rust or admin-web work.
- **No native config changes**: no Info.plist / AndroidManifest edits, no new pub deps, no `pod install`.
- **Tests**: `history_screen_test.dart` is new (~5 testWidgets). Existing tests stay green; queue processor test gets one new case for the recently-synced provider push.
- **App size**: no measurable impact.
- **Risk**: low — this is purely additive UX. Worst case rollback is reverting two providers, two widget hooks, and the dialog. The only behavior change to existing flows is "tapping logout while queue non-empty now shows a dialog first" — easy to undo if it backfires.
