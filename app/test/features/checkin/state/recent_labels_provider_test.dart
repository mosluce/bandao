import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/app_user.dart';
import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/core/api/models/org.dart';
import 'package:bandao_app/core/storage/secure_storage.dart';
import 'package:bandao_app/features/auth/state/auth_provider.dart';
import 'package:bandao_app/features/auth/state/auth_state.dart';
import 'package:bandao_app/features/checkin/data/checkin_repository.dart';
import 'package:bandao_app/features/checkin/data/recent_labels.dart';
import 'package:bandao_app/features/checkin/state/recent_labels_provider.dart';

import '../../../helpers/fake_auth_notifier.dart';
import '../../../helpers/fake_secure_storage.dart';

AuthState _authed(String userId) => AuthState.authenticated(
      user: AppUser(
        id: userId,
        username: userId,
        displayName: userId,
        status: AppUserStatus.active,
        needsPasswordChange: false,
        createdAt: '2025-01-01T00:00:00Z',
      ),
      org: Org(
        id: 'o1',
        name: 'Acme',
        code: 'ABCDEFGHIJ',
        ownerId: 'u1',
        timezone: 'Asia/Taipei',
        checkin: OrgCheckin(transferEnabled: true),
      ),
      needsPasswordChange: false,
    );

class _FakeRepo implements CheckinRepository {
  _FakeRepo({this.events_ = const <CheckinEventDto>[], this.throws = false});

  final List<CheckinEventDto> events_;
  final bool throws;

  @override
  Future<List<CheckinEventDto>> events({
    String? before,
    int limit = 50,
    DateTime? from,
    DateTime? to,
  }) async {
    if (throws) throw StateError('offline');
    return events_;
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName} not stubbed');
}

CheckinEventDto _ev(String label, {Duration ago = const Duration(hours: 1)}) {
  final at = DateTime.now().subtract(ago).toIso8601String();
  return CheckinEventDto(
    id: 'e$label${ago.inMinutes}',
    appUserId: 'u1',
    eventType: CheckinEventType.clockIn,
    occurredAtClient: at,
    occurredAtServer: at,
    source: EventSource.app,
    initiatedByKind: EventInitiatorKind.appUser,
    initiatedById: 'u1',
    location: EventLocation(
      coordinates: const GeoPoint(lat: 25, lng: 121),
      manualLabel: label,
    ),
    hasSkewWarning: false,
  );
}

ProviderContainer _container({
  required CheckinRepository repo,
  required FakeSecureStorage storage,
  String userId = 'u1',
}) {
  final c = ProviderContainer(
    overrides: <Override>[
      checkinRepositoryProvider.overrideWith((ref) async => repo),
      secureStorageProvider.overrideWithValue(storage),
      authProvider.overrideWith(
        () => FakeAuthNotifier(AsyncValue.data(_authed(userId))),
      ),
    ],
  );
  addTearDown(c.dispose);
  return c;
}

/// Build a container and wait for `authProvider` to resolve.
///
/// `FakeAuthNotifier.build` is async, so before the first microtask the
/// signed-in id reads as null and the labels provider correctly returns an
/// empty list. Production goes through the same ordering — the labels
/// provider rebuilds once auth resolves — but a test that reads immediately
/// would be asserting against the pre-auth frame.
Future<ProviderContainer> _ready({
  required CheckinRepository repo,
  required FakeSecureStorage storage,
  String userId = 'u1',
}) async {
  final c = _container(repo: repo, storage: storage, userId: userId);
  await c.read(authProvider.future);
  return c;
}

void main() {
  group('RecentLabelsNotifier', () {
    test('derives suggestions from the AppUser own history', () async {
      final c = await _ready(
        repo: _FakeRepo(events_: [_ev('甲工地'), _ev('甲工地'), _ev('乙工地')]),
        storage: FakeSecureStorage(),
      );

      expect(await c.read(recentLabelsProvider.future), ['甲工地', '乙工地']);
    });

    // The gap found on a device: after a check-in the chip did not appear,
    // because the event sits in the local queue and the server does not know
    // about it yet. Refetching cannot fix that; the device has to remember.
    test('remembers a label immediately, without a server round-trip',
        () async {
      final c = await _ready(
        repo: _FakeRepo(events_: const []),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);
      expect(c.read(recentLabelsProvider).value, isEmpty);

      await c.read(recentLabelsProvider.notifier).remember('新工地');

      expect(c.read(recentLabelsProvider).value, ['新工地']);
    });

    test('a remembered label works offline too', () async {
      final c = await _ready(
        repo: _FakeRepo(throws: true),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('離線工地');

      expect(c.read(recentLabelsProvider).value, ['離線工地']);
    });

    test('re-using a label promotes it without duplicating', () async {
      final c = await _ready(
        repo: _FakeRepo(events_: [_ev('甲工地'), _ev('乙工地')]),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('乙工地');

      expect(c.read(recentLabelsProvider).value, ['乙工地', '甲工地']);
    });

    test('remembering respects the cap', () async {
      final c = await _ready(
        repo: _FakeRepo(
          events_: [for (var i = 0; i < 6; i++) _ev('site$i')],
        ),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('新工地');

      final v = c.read(recentLabelsProvider).value!;
      expect(v, hasLength(recentLabelsCap));
      expect(v.first, '新工地');
    });

    test('a remembered label is cached so it survives a restart', () async {
      final storage = FakeSecureStorage();
      final c = await _ready(repo: _FakeRepo(throws: true), storage: storage);
      await c.read(recentLabelsProvider.future);
      await c.read(recentLabelsProvider.notifier).remember('離線工地');

      final cached = await storage.readRecentCheckinLabels('u1');
      expect(jsonDecode(cached!), ['離線工地']);

      // A fresh container with the same storage, still offline.
      final c2 = await _ready(repo: _FakeRepo(throws: true), storage: storage);
      expect(await c2.read(recentLabelsProvider.future), ['離線工地']);
    });

    // Reported from a device: signing in as a different worker on the same
    // phone showed the previous worker's site names as chips. Devices are
    // shared here — it is why `wipeForOtherUsers` exists for the event queue
    // — and the labels are customer names, so this was a cross-account leak.
    test('one AppUser never sees another AppUser cached labels', () async {
      final storage = FakeSecureStorage();

      final first = await _ready(
        repo: _FakeRepo(throws: true),
        storage: storage,
        userId: 'worker-a',
      );
      await first.read(recentLabelsProvider.future);
      await first.read(recentLabelsProvider.notifier).remember('甲工地');
      expect(first.read(recentLabelsProvider).value, ['甲工地']);

      // Same device, same storage, different signed-in AppUser.
      final second = await _ready(
        repo: _FakeRepo(throws: true),
        storage: storage,
        userId: 'worker-b',
      );

      expect(
        await second.read(recentLabelsProvider.future),
        isEmpty,
        reason: 'worker-b must not inherit worker-a labels',
      );
    });

    test('each AppUser keeps their own cached labels', () async {
      final storage = FakeSecureStorage();

      final a = await _ready(
        repo: _FakeRepo(throws: true),
        storage: storage,
        userId: 'worker-a',
      );
      await a.read(recentLabelsProvider.future);
      await a.read(recentLabelsProvider.notifier).remember('甲工地');

      final b = await _ready(
        repo: _FakeRepo(throws: true),
        storage: storage,
        userId: 'worker-b',
      );
      await b.read(recentLabelsProvider.future);
      await b.read(recentLabelsProvider.notifier).remember('乙工地');

      // Re-reading A's cache must still yield A's own label.
      final aAgain = await _ready(
        repo: _FakeRepo(throws: true),
        storage: storage,
        userId: 'worker-a',
      );
      expect(await aAgain.read(recentLabelsProvider.future), ['甲工地']);
    });

    test('no session offers nothing rather than the last user labels',
        () async {
      final storage = FakeSecureStorage();
      final signedIn = await _ready(
        repo: _FakeRepo(throws: true),
        storage: storage,
        userId: 'worker-a',
      );
      await signedIn.read(recentLabelsProvider.future);
      await signedIn.read(recentLabelsProvider.notifier).remember('甲工地');

      final loggedOut = ProviderContainer(
        overrides: <Override>[
          checkinRepositoryProvider
              .overrideWith((ref) async => _FakeRepo(throws: true)),
          secureStorageProvider.overrideWithValue(storage),
          authProvider.overrideWith(
            () => FakeAuthNotifier(
              const AsyncValue.data(AuthState.unauthenticated()),
            ),
          ),
        ],
      );
      addTearDown(loggedOut.dispose);

      expect(await loggedOut.read(recentLabelsProvider.future), isEmpty);
    });

    test('an empty label is ignored', () async {
      final c = await _ready(
        repo: _FakeRepo(events_: const []),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('   ');

      expect(c.read(recentLabelsProvider).value, isEmpty);
    });
  });
}
