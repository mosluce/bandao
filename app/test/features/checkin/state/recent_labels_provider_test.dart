import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/core/storage/secure_storage.dart';
import 'package:bandao_app/features/checkin/data/checkin_repository.dart';
import 'package:bandao_app/features/checkin/data/recent_labels.dart';
import 'package:bandao_app/features/checkin/state/recent_labels_provider.dart';

import '../../../helpers/fake_secure_storage.dart';

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
}) {
  final c = ProviderContainer(
    overrides: <Override>[
      checkinRepositoryProvider.overrideWith((ref) async => repo),
      secureStorageProvider.overrideWithValue(storage),
    ],
  );
  addTearDown(c.dispose);
  return c;
}

void main() {
  group('RecentLabelsNotifier', () {
    test('derives suggestions from the AppUser own history', () async {
      final c = _container(
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
      final c = _container(
        repo: _FakeRepo(events_: const []),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);
      expect(c.read(recentLabelsProvider).value, isEmpty);

      await c.read(recentLabelsProvider.notifier).remember('新工地');

      expect(c.read(recentLabelsProvider).value, ['新工地']);
    });

    test('a remembered label works offline too', () async {
      final c = _container(
        repo: _FakeRepo(throws: true),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('離線工地');

      expect(c.read(recentLabelsProvider).value, ['離線工地']);
    });

    test('re-using a label promotes it without duplicating', () async {
      final c = _container(
        repo: _FakeRepo(events_: [_ev('甲工地'), _ev('乙工地')]),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('乙工地');

      expect(c.read(recentLabelsProvider).value, ['乙工地', '甲工地']);
    });

    test('remembering respects the cap', () async {
      final c = _container(
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
      final c = _container(repo: _FakeRepo(throws: true), storage: storage);
      await c.read(recentLabelsProvider.future);
      await c.read(recentLabelsProvider.notifier).remember('離線工地');

      final cached = await storage.readRecentCheckinLabels();
      expect(jsonDecode(cached!), ['離線工地']);

      // A fresh container with the same storage, still offline.
      final c2 = _container(repo: _FakeRepo(throws: true), storage: storage);
      expect(await c2.read(recentLabelsProvider.future), ['離線工地']);
    });

    test('an empty label is ignored', () async {
      final c = _container(
        repo: _FakeRepo(events_: const []),
        storage: FakeSecureStorage(),
      );
      await c.read(recentLabelsProvider.future);

      await c.read(recentLabelsProvider.notifier).remember('   ');

      expect(c.read(recentLabelsProvider).value, isEmpty);
    });
  });
}
