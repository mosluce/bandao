import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/trajectory/data/trajectory_stats.dart';

LocationPingDto _ping({
  required double lat,
  required double lng,
  required String occurredAtClient,
}) {
  return LocationPingDto(
    id: 'x',
    appUserId: 'u',
    lat: lat,
    lng: lng,
    occurredAtClient: occurredAtClient,
    occurredAtServer: occurredAtClient,
  );
}

void main() {
  group('computeTrajectoryStats', () {
    test('empty input returns the empty sentinel', () {
      expect(computeTrajectoryStats(const []), same(TrajectoryStats.empty));
    });

    test('single ping has zero distance, zero duration', () {
      final stats = computeTrajectoryStats([
        _ping(lat: 25.0, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
      ]);
      expect(stats.distanceMeters, 0);
      expect(stats.onShiftDuration, Duration.zero);
      expect(stats.pingCount, 1);
    });

    test('two pings ~100m apart give a non-trivial distance', () {
      // ~0.001 degree of latitude ≈ 111 metres.
      final stats = computeTrajectoryStats([
        _ping(lat: 25.000, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
        _ping(lat: 25.001, lng: 121.0, occurredAtClient: '2026-05-15T09:05:00Z'),
      ]);
      expect(stats.distanceMeters, greaterThan(100));
      expect(stats.distanceMeters, lessThan(120));
      expect(stats.onShiftDuration, const Duration(minutes: 5));
      expect(stats.pingCount, 2);
    });

    test('unsorted input is sorted ascending before distance sum', () {
      final sortedFirst = computeTrajectoryStats([
        _ping(lat: 25.000, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
        _ping(lat: 25.001, lng: 121.0, occurredAtClient: '2026-05-15T09:05:00Z'),
        _ping(lat: 25.002, lng: 121.0, occurredAtClient: '2026-05-15T09:10:00Z'),
      ]);
      final unsorted = computeTrajectoryStats([
        _ping(lat: 25.002, lng: 121.0, occurredAtClient: '2026-05-15T09:10:00Z'),
        _ping(lat: 25.000, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
        _ping(lat: 25.001, lng: 121.0, occurredAtClient: '2026-05-15T09:05:00Z'),
      ]);
      // Both should produce ~the same distance — order of input must not
      // matter once we sort by occurred_at_client.
      expect(
        (unsorted.distanceMeters - sortedFirst.distanceMeters).abs(),
        lessThan(0.5),
      );
      expect(unsorted.onShiftDuration, const Duration(minutes: 10));
    });

    test('duration is clamped to zero if timestamps somehow regress', () {
      // Defensive: identical timestamps after sort -> zero duration, not
      // a negative one.
      final stats = computeTrajectoryStats([
        _ping(lat: 25.0, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
        _ping(lat: 25.0, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
      ]);
      expect(stats.onShiftDuration, Duration.zero);
    });
  });
}
