## Context

`add-app-checkin` shipped a working mobile checkin client that survived live smoke against the existing `/app/checkin/*` surface — multi-site flow, airplane-mode queue + drain, failed event flow, and login-handover wipe all passed. Four issues surfaced during smoke that didn't block archive but degrade the daily UX:

1. Just-synced events disappear from `/history` until manual refresh.
2. Buttons for `[轉出]` / `[轉入]` render even when org has disabled transfer; users discover this via a `failed` row.
3. Logout fires immediately even when queue holds events that will be wiped on a different-user login.
4. `/me` and `/app/checkin/status` only fetched at login; admin-side flips are invisible until cold start.

Plus the deferred history widget test (item 14.8) — bundled here because items 1 and 4 change the surface that test would cover, and writing it alongside the change is cheaper than after.

This change is purely mobile-client polish. No API changes, no native config, no new dependencies.

## Goals / Non-Goals

**Goals:**

- Eliminate the "vanishing event" perception in `/history` — once a tap turns into a server-confirmed event, the row stays visible without requiring user action.
- Match the visible button set to the actual server contract — if `transfer_enabled = false`, don't expose buttons that will fail.
- Give the user a chance to abort logout when their queue still has work in flight, without changing the queue-wipe semantics on different-user login.
- Keep cached org settings + checkin status fresh enough that admin-side flips reflect within seconds of the user opening the app.
- Let the user manually pull-to-refresh `/history` when they want to confirm sync state — common gesture, low-friction safety valve.

**Non-Goals:**

- AnimatedList / AnimatedSwitcher transitions for the just-synced row swap. Straight `ListView` rebuild is acceptable; visual smoothness is a separate item if real users complain.
- Background polling of `/me` or `/app/checkin/status` while the app is foreground. Refresh is event-driven (login, app-resume, manual pull), not polled.
- Coalescing rapid app-resume refreshes. If the user repeatedly backgrounds and foregrounds within seconds, we'll fire `/me` each time — observed-then-fixed, not pre-empted.
- Server-side `transfer_enabled` enforcement reshaping. Server already returns `TRANSFER_DISABLED`; this change only adjusts the client UX.
- Persisting recently-synced events across app restarts. The cache is in-memory only; a cold start refetches the first server page anyway.

## Decisions

### Three-source merge in `/history`

The just-synced event problem has three viable solutions (refetch-on-shrink, in-memory recently-synced cache, mark-synced-in-DB). We pick **the in-memory cache (option B)** for these reasons:

- **Zero extra network**. The submit response already carries the full `CheckinEventDto`; we just plumb it to a provider.
- **No schema change**. Drift's `pending_events` table stays single-purpose: rows that the server has not yet confirmed. Synced rows leave the queue.
- **Aligns with the "queue is the source of truth for in-flight" model**. The recently-synced cache is a UI-side hint, not authoritative — a hard refresh wipes it and shows the canonical server state.
- **Simple dedupe**. The history merge can dedupe by event `id`. When the user paginates with `[載入更多]` and the server returns an event already in the cache, the cache entry drops.

Architecture:

```
┌───────────────────────┐
│ QueueProcessor        │
│  on submit() success: │
│    delete row         │
│    push status fresh  │
│    push event fresh ──┼──▶ recentlySyncedEventsProvider (StateProvider<List<CheckinEventDto>>)
└───────────────────────┘                          │
                                                    │
┌───────────────────────┐                           │
│ HistoryScreen         │                           │
│  merges:              │                           │
│    queue (drift watch)◀───── checkinQueueProvider │
│    server (paginated) ◀───── _serverEvents (state)│
│    recently-synced    ◀───────────────────────────┘
│  sort by occurred_at_client desc                  │
│  dedupe by event.id (server wins over cache)      │
└───────────────────────┘
```

The provider lives in `app/lib/features/checkin/state/recently_synced_events_provider.dart`. Bounded growth: the cache holds at most 50 events (matches the server page size); when the user pulls-to-refresh or paginates and the cache entries are now in `_serverEvents`, they fall out of the merged view via dedupe but stay in the provider until cleared. To prevent unbounded growth in degenerate cases (user submits 100s of events without ever opening history), cap the provider at 50 — drop oldest on push.

We considered marking rows `synced` in drift instead of deleting, but it forces a schema migration just for a UI affordance, and complicates the queue processor's state machine (now `pending | sending | failed | synced` instead of three).

### `HomeButtons` reads `auth.org.checkin.transferEnabled`

The cached `Org` is on the auth state — already wired into the home screen via `ref.watch(authProvider)`. `HomeButtons` already reads the effective status; adding a transferEnabled check is a one-line gate inside the on_site / in_transit branches. No new provider, no extra fetch.

When `transferEnabled = false`:

```
off_duty   → [上班]
on_site    → [下班]                  (drops [轉出])
in_transit → [下班]                  (drops [轉入])
```

The in_transit + transfer-disabled cell is the awkward one — user can only clock out, can't reach the next site. The server's state-lock should prevent this in practice (admin can't flip while anyone is non-off_duty), but if it does happen (lock bypass, force-flip), `[下班]` is the correct legal action and the user is back to off_duty afterward.

We chose **trust the cached value** rather than fetch fresh on render. Staleness is closed by the app-resume refresh below — admin's flip becomes visible the moment the user brings the app to foreground. We could pre-empt with a periodic refresh or a server-pushed signal, but neither is justified for v1.

### Logout confirmation gates the menu action

The logout menu's `onSelected` handler becomes async:

```
on tap '登出':
  read queue rows for current user
  count = total rows (pending + sending + failed)
  if count == 0:
    proceed with authProvider.logout()  (existing behavior)
  else:
    show confirm dialog with count and "登出後若由其他帳號登入，這些事件會被清除"
    if user confirms:
      proceed with authProvider.logout()
    else:
      no-op
```

We pick **inclusive count** (pending + sending + failed) because all three classes get wiped on different-user login per the existing `wipeForOtherUsers` semantics. Excluding `failed` would understate the data-loss surface; the user might assume "I dismissed the failures, my queue is clean" when in fact 5 silently-stuck failed rows are about to vanish. Inclusive matches the actual threat.

The dialog uses `showDialog<bool>` returning the user's choice. We do not change `authProvider.logout()` itself — it stays "force-clear local state regardless of network", because once the user has confirmed, the wipe semantics are unchanged.

### App resume refreshes `/me` + status, not just permission

`home_screen.dart` already has `WidgetsBindingObserver` wired for permission re-check. We add two more refreshes on the `AppLifecycleState.resumed` branch:

```dart
case AppLifecycleState.resumed:
  ref.read(locationPermissionProvider.notifier).refresh();
  ref.read(authProvider.notifier).refreshMe();      // NEW
  ref.read(checkinStatusProvider.notifier).refresh(); // NEW (already has refresh())
```

`AuthNotifier.refreshMe()` is new — internally just calls `_fetchMe()` and replaces state. It does NOT re-run the handover wipe (the wipe is a login-time guard against device handoff; an app-resume isn't a login event). It does refresh `Org.checkin.transferEnabled`, which closes the staleness window for the button-hide logic.

Risk: rapid background/foreground cycles fire `/me` each time. We accept this — `/me` is cheap, the user agent caches the bearer header, and rapid cycling isn't a real-world pattern. If observed, add a 5-second debounce.

### History pull-to-refresh

`RefreshIndicator` wraps the existing `ListView.separated`. The `onRefresh` handler:

1. Clear `recentlySyncedEventsProvider` (reset to empty list — server will be authoritative after the refetch).
2. Reset `_serverEvents = []`, `_hasMore = true`, `_error = null`.
3. Call the existing `_loadMore()` to fetch the first page (now with empty `before`).
4. Refresh `checkinStatusProvider` so the status pill updates if the user pulled because something looked stale.

Local queue stream is live via `StreamProvider`, so it doesn't need explicit refresh.

This gives the user a single, well-known gesture to assert "show me the canonical server state right now" — useful when they're not sure if a sync went through, or after they've done admin-side actions and want to see the result reflected.

### History widget test scope

Five `testWidgets` covering the merge surface:

1. **pending + synced render together** — sanity, mirrors the existing `effective_status_provider_test.dart` scenario.
2. **Just-synced row appears in place after the recently-synced provider emits** — the new path. Override `recentlySyncedEventsProvider` mid-pump and verify the row is visible.
3. **Failed row dismiss flow** — tap `[關閉]`, verify the row disappears (drift-backed delete).
4. **`[載入更多]` triggers paginated server fetch** — fake repo records the `before` argument; verify it equals the oldest currently-displayed `occurred_at_client`.
5. **Pull-to-refresh resets state** — fire the refresh callback (gesture or direct), verify recently-synced cache is cleared and a fresh first-page fetch fires (`before` is null).

We use the same lean override pattern as the queue chip test (Stream<List<QueueRow>> override + fake repo via Riverpod), avoiding the drift in-memory DB pattern that triggered the timer-leak issues.

## Risks / Trade-offs

- **Three-source merge can briefly double-render an event** if the recently-synced cache and `_serverEvents` both contain the same `id` between push and dedupe. Mitigation: dedupe runs every render (cheap, list of ~50). Net effect: one frame of double-render, indistinguishable from a normal re-render.
- **Bounded recently-synced cache (50 events)** could drop a sync if the user submits 50+ events without refreshing or paginating. Mitigation: practically never happens (50 events is a long shift); even if it does, the dropped event is still on the server and `[載入更多]` will fetch it.
- **Cached `Org.checkin.transferEnabled` staleness window** — admin flips, user is on home looking at buttons. Until they background+foreground (or manually pull-to-refresh), the buttons reflect the old value. Acceptable: server still rejects with `TRANSFER_DISABLED`, the worst outcome is one failed row + clear error message, which the user can dismiss.
- **Logout confirm dialog interrupts a power-user flow.** A user who *intends* to sign out + sign back in as themselves (token issues, etc) hits the dialog every time. Mitigation: the dialog is informational + 1 tap to confirm. If feedback says it's annoying for the same-user case, we can short-circuit — but that requires asking "who are you about to log in as?" which we don't know yet, so we conservatively show the warning.
- **App resume refresh hammers `/me` if the user rapidly toggles**. Mitigation: documented limitation; debounce only if observed.
- **Pull-to-refresh resets `_hasMore`**. If the user had loaded 200 paginated events, pull-to-refresh drops them and starts from the top. Mitigation: this is the intended semantic — `RefreshIndicator` traditionally means "give me the freshest first page". Users wanting "more" use `[載入更多]`.
- **Three-source dedupe by `id` requires server `id` for synced events.** All `CheckinEventDto` rows have `id` server-side. The recently-synced cache holds `CheckinEventDto`, so the field is present. Local queue rows have synthesized `queue#<id>` references; they stay separate from the dedupe key (queue rows are matched by `(occurred_at_client, app_user_id)` proximity if needed, but in practice they never collide because queue rows are deleted before their server counterpart appears).

## Migration Plan

No data migration. No schema changes. No native config.

For developers:

1. Pull, run `flutter pub get` (no new packages).
2. Hot-restart the existing build — the change is dart-only.
3. CI runs the existing test pipeline + the new history widget test.

For end-users:

- The change is invisible at app start. The first time they submit an event after upgrading, the synced row appears in `/history` instead of vanishing. The first time they tap logout with a non-empty queue, they see the new confirm dialog. The first time they pull down on `/history`, they get a refresh.

No rollback plan needed pre-launch.

## Open Questions

- **Should pull-to-refresh also re-check location permission?** Borderline — the gesture's purpose is "refresh server state", not "re-check OS state". Probably leave it off; the resume-hook already covers permission, and adding more side-effects to a gesture muddies the intent.
- **Should the logout confirm count *only* show pending + sending (excluding failed)?** Decided no above — failed rows are wiped same as pending on different-user login, so the count needs to reflect actual loss. But if user feedback says "I dismissed those, why are they in the count", we revisit.
- **Should `refreshMe()` be invalidate-and-refetch or in-place update?** In-place is faster (no loading flicker on the home shell) but a failed refresh might leave a stale state. Going with in-place for now (matches `updateFromServer` pattern on the status provider). If a failed `/me` corrupts the home shell (e.g., shows wrong org name briefly), switch to invalidate.
