## 1. Recently-synced events provider

- [x] 1.1 Create `app/lib/features/checkin/state/recently_synced_events_provider.dart` with a `Notifier<List<CheckinEventDto>>` that exposes `push(CheckinEventDto)`, `clear()`, and a 50-entry cap (drop oldest on push beyond cap).
- [x] 1.2 Wire `queueProcessorProvider` to call `ref.read(recentlySyncedEventsProvider.notifier).push(response.event)` on every successful submit alongside the existing `onStatusFresh` callback. Add a new `onEventSynced` callback param to `QueueProcessor`.

## 2. History merge: 3 sources + dedupe

- [x] 2.1 Update `HistoryScreen._mergeAndSort` to accept three sources: queue, server, recently-synced. After merging, dedupe by event `id` — when a server-fetched event and a recently-synced event share the same `id`, keep the server-fetched one (it has authoritative `region_name` etc.). Local queue rows have no server `id` so they don't participate in dedupe.
- [x] 2.2 Watch `recentlySyncedEventsProvider` in `HistoryScreen.build`; pass its value into `_mergeAndSort`.
- [x] 2.3 Verify that when a queue row is deleted (sync success), the recently-synced cache emission keeps a row at the same `occurred_at_client` visible — straight `ListView` rebuild, no animation.

## 3. HomeButtons honors `transferEnabled`

- [x] 3.1 In `home_buttons.dart`, read `auth.org.checkin.transferEnabled` from `ref.watch(authProvider).valueOrNull`. When `false`, drop `[轉出]` from the on_site set and `[轉入]` from the in_transit set; off_duty is unaffected.
- [x] 3.2 Update the location-permission gate to disable the visible buttons only when permission is `LocationPermission.deniedForever` (already the runtime behavior; align comment + structure to the spec wording).
- [x] 3.3 Sync `LocationPermissionBlocker` similarly — only render when permission is `deniedForever`. (Already the runtime behavior; this is a no-op verification.)

## 4. Logout confirm dialog

- [x] 4.1 In `home_screen.dart`, change the popup-menu `onSelected('logout')` handler to async. Read `checkinQueueProvider`, count rows for the current user across pending + sending + failed.
- [x] 4.2 If count > 0, `showDialog<bool>` with the spec'd copy (`你還有 N 筆事件未處理。登出後若由其他帳號登入，這些事件會被清除。確定要登出？`) and `[取消]` / `[仍要登出]` actions. Only call `authProvider.logout()` on confirm.
- [x] 4.3 If count == 0, fall through to the existing immediate `authProvider.logout()` (no dialog).
- [x] 4.4 Add localization strings: `logoutConfirmTitle`, `logoutConfirmBody(int n)`, `logoutConfirmCancel`, `logoutConfirmProceed`.

## 5. App resume refresh

- [x] 5.1 Add `AuthNotifier.refreshMe()` to `auth_provider.dart`. Internally calls a thin variant of `_fetchMe()` that updates state in place but does NOT run the handover wipe (resume is not a login event). Document the distinction in a code comment.
- [x] 5.2 In `home_screen.dart`'s `didChangeAppLifecycleState(resumed)`, add `ref.read(authProvider.notifier).refreshMe()` and `ref.read(checkinStatusProvider.notifier).refresh()` alongside the existing permission re-check.
- [x] 5.3 Verify `_fetchMe()` (login path) and `refreshMe()` (resume path) share the underlying repo call but split on whether the wipe runs — extract a private helper if it cleans up.

## 6. History pull-to-refresh

- [x] 6.1 In `history_screen.dart`, wrap the existing `ListView.separated` in a `RefreshIndicator`. The `onRefresh` callback resets `_serverEvents = []`, `_hasMore = true`, `_error = null`, calls `recentlySyncedEventsProvider.notifier.clear()`, and awaits `_loadMore()` (which now fetches the first page since `_serverEvents.isEmpty`).
- [x] 6.2 Also call `ref.read(checkinStatusProvider.notifier).refresh()` in the `onRefresh` callback so the user gets a fresh status alongside the fresh page.
- [x] 6.3 Empty-state visual: the existing `Center(child: Text(historyEmpty))` when `entries.isEmpty && !_loading` doesn't support pull — wrap that branch's child in a `SingleChildScrollView` + `RefreshIndicator` so the gesture works even with no items.

## 7. Tests

- [x] 7.1 `test/features/checkin/state/recently_synced_events_provider_test.dart`: push N+1 entries, verify oldest dropped at cap; clear empties the list.
- [x] 7.2 Update `test/features/checkin/data/queue_processor_test.dart`: add a case verifying `onEventSynced` callback is invoked with the response's `event` on 201 success.
- [x] 7.3 Update `test/features/checkin/presentation/home_buttons_test.dart`: add cases for `transferEnabled = false` collapsing on_site to `[下班]` only and in_transit to `[下班]` only. Off_duty unchanged.
- [x] 7.4 Create `test/features/checkin/presentation/history_screen_test.dart`: 5 testWidgets covering pending+synced merge, just-synced row appearing in place via the recently-synced provider, failed-row dismiss, `[載入更多]` triggers paginated fetch with the correct `before` cursor, pull-to-refresh resets state. Use the lean override pattern (Stream<List<QueueRow>>, fake repo via Riverpod) — no in-memory drift DB.
- [x] 7.5 Update `test/features/checkin/presentation/location_permission_blocker_test.dart`: verify the `denied` (never-asked) state hides the blocker; only `deniedForever` shows it.
- [x] 7.6 Add a logout-confirm widget test (could live in a new `test/features/auth/presentation/home_screen_logout_test.dart` or fold into an existing home test): non-empty queue → dialog appears; cancel → no logout call; confirm → logout called.

## 8. Localization

- [x] 8.1 Add the four logout-confirm strings to `app/lib/l10n/app_localizations.dart` (zh-TW + English shadow).
- [x] 8.2 No changes needed for existing strings (button hide reuses `eventTransferOut/In`; pull-to-refresh uses platform default indicator).

## 9. Docs

- [x] 9.1 Append a short "Polish iteration" note to `app/README.md` summarizing the new behaviors (just-synced visibility in history, transfer-button hide, logout confirm, resume refresh, pull-to-refresh) — one paragraph.

## 10. Smoke

- [x] 10.1 `flutter analyze` clean, `flutter test` all green locally.
- [x] 10.2 Live smoke on iPhone Simulator: submit an event, watch it appear in `/history` immediately as `已上傳` (no disappear-and-reappear). Toggle Wi-Fi off, submit two events offline, toggle on; verify both flip from pending to synced in place.
- [x] 10.3 Live smoke: in admin-web flip `transfer_enabled = false`, bring app to foreground, verify on-site button set collapses to `[下班]` only. Flip back to `true`, foreground, verify `[轉出]` returns.
- [x] 10.4 Live smoke: with a non-empty queue (offline, tap a button), trigger `登出` from the home `…` menu; verify the confirmation dialog appears with the correct count; tap `取消` and verify nothing changes; tap `登出` again and tap `仍要登出` to confirm; verify logout proceeds.
- [x] 10.5 Live smoke: pull down on `/history` — first page refetches, recently-synced cache resets, status refreshes. Verify queue rows stay visible during the pull.
- [ ] 10.6 Live smoke: background app while a queue row is `pending`, wait for the server to receive it (via background sync OR by force-quitting and reopening), foreground; verify the home pill reflects the post-event status (proves resume refresh works). *Skipped at user's request — resume refresh wired and unit-tested via 5.x; cleanest live verification path is admin `force-checkout` while app is backgrounded, deferred to a later session.*
