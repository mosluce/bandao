import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/api_error.dart';
import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/checkin/data/checkin_repository.dart';
import 'package:bandao_app/features/trajectory/data/my_locations_repository.dart';
import 'package:bandao_app/features/trajectory/state/trajectory_controller.dart';

/// The controller fetches the day's events for the start anchor; stub to empty.
class _StubCheckinRepo implements CheckinRepository {
  @override
  Future<List<CheckinEventDto>> events({String? before, int limit = 50}) async =>
      const <CheckinEventDto>[];

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName} not stubbed');
}

class _FakeMyLocationsRepository implements MyLocationsRepository {
  _FakeMyLocationsRepository(this._impl);

  final Future<List<LocationPingDto>> Function({
    required DateTime from,
    required DateTime to,
    int? limit,
  }) _impl;

  final List<({DateTime from, DateTime to})> calls = [];

  @override
  Future<List<LocationPingDto>> listForRange({
    required DateTime from,
    required DateTime to,
    int? limit,
  }) async {
    calls.add((from: from, to: to));
    return _impl(from: from, to: to, limit: limit);
  }
}

LocationPingDto _ping(String iso, {double lat = 25.0, double lng = 121.0}) {
  return LocationPingDto(
    id: 'x',
    appUserId: 'u',
    lat: lat,
    lng: lng,
    occurredAtClient: iso,
    occurredAtServer: iso,
  );
}

ProviderContainer _container(_FakeMyLocationsRepository repo) {
  return ProviderContainer(
    overrides: [
      myLocationsRepositoryProvider.overrideWith((ref) async => repo),
      checkinRepositoryProvider.overrideWith((ref) async => _StubCheckinRepo()),
    ],
  );
}

void main() {
  group('TrajectoryController', () {
    test('build fetches today and computes stats', () async {
      final repo = _FakeMyLocationsRepository(({
        required from,
        required to,
        int? limit,
      }) async {
        return [
          _ping('2026-05-15T09:00:00Z', lat: 25.000),
          _ping('2026-05-15T09:05:00Z', lat: 25.001),
        ];
      });
      final container = _container(repo);
      addTearDown(container.dispose);

      final state = await container.read(trajectoryProvider.future);

      expect(repo.calls.length, 1);
      // Range spans exactly one day from the local-midnight `selectedDate`.
      final span = repo.calls.first.to.difference(repo.calls.first.from);
      expect(span, const Duration(days: 1));
      // selectedDate is the local midnight of today.
      final now = DateTime.now();
      expect(state.selectedDate.year, now.year);
      expect(state.selectedDate.month, now.month);
      expect(state.selectedDate.day, now.day);
      expect(state.selectedDate.hour, 0);

      expect(state.pings.length, 2);
      expect(state.stats.pingCount, 2);
      expect(state.stats.onShiftDuration, const Duration(minutes: 5));
    });

    test('selectDate triggers a refetch for the new day', () async {
      final repo = _FakeMyLocationsRepository(({
        required from,
        required to,
        int? limit,
      }) async {
        return [];
      });
      final container = _container(repo);
      addTearDown(container.dispose);

      await container.read(trajectoryProvider.future);
      final initialCalls = repo.calls.length;

      final earlier = DateTime(2026, 5, 14, 10, 30);
      await container.read(trajectoryProvider.notifier).selectDate(earlier);
      final state = container.read(trajectoryProvider).value!;

      expect(repo.calls.length, initialCalls + 1);
      expect(state.selectedDate, DateTime(2026, 5, 14));
      expect(repo.calls.last.from, DateTime(2026, 5, 14));
      expect(repo.calls.last.to, DateTime(2026, 5, 15));
      expect(state.pings, isEmpty);
      expect(state.stats.pingCount, 0);
    });

    test('repository error surfaces as AsyncError', () async {
      final repo = _FakeMyLocationsRepository(({
        required from,
        required to,
        int? limit,
      }) async {
        throw ApiException.network('offline');
      });
      final container = _container(repo);
      addTearDown(container.dispose);

      await expectLater(
        () => container.read(trajectoryProvider.future),
        throwsA(isA<ApiException>()),
      );
      final state = container.read(trajectoryProvider);
      expect(state.hasError, isTrue);
    });

    test('refresh re-queries the currently-selected day', () async {
      var callCount = 0;
      final repo = _FakeMyLocationsRepository(({
        required from,
        required to,
        int? limit,
      }) async {
        callCount += 1;
        return [];
      });
      final container = _container(repo);
      addTearDown(container.dispose);

      await container.read(trajectoryProvider.future);
      expect(callCount, 1);

      await container.read(trajectoryProvider.notifier).refresh();
      expect(callCount, 2);
      // Both calls span the same day (today's local midnight).
      expect(repo.calls.first.from, repo.calls.last.from);
    });
  });
}
