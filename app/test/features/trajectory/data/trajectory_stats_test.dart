import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/features/trajectory/data/trajectory_merge.dart';
import 'package:bandao_app/features/trajectory/data/trajectory_stats.dart';

MergedPoint _pt({
  required double lat,
  required double lng,
  required String occurredAtClient,
  int originRank = originPing,
  String id = 'x',
}) {
  return MergedPoint(
    lat: lat,
    lng: lng,
    occurredAtClient: occurredAtClient,
    originRank: originRank,
    id: id,
  );
}

void main() {
  group('computeTrajectoryStats', () {
    test('empty input returns the empty sentinel', () {
      expect(computeTrajectoryStats(const []), same(TrajectoryStats.empty));
    });

    test('single point has zero distance, zero duration', () {
      final stats = computeTrajectoryStats([
        _pt(lat: 25.0, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
      ]);
      expect(stats.distanceMeters, 0);
      expect(stats.onShiftDuration, Duration.zero);
      expect(stats.pointCount, 1);
    });

    test('two points ~100m apart give a non-trivial distance', () {
      // ~0.001 degree of latitude ≈ 111 metres.
      final stats = computeTrajectoryStats([
        _pt(lat: 25.000, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
        _pt(lat: 25.001, lng: 121.0, occurredAtClient: '2026-05-15T09:05:00Z'),
      ]);
      expect(stats.distanceMeters, greaterThan(100));
      expect(stats.distanceMeters, lessThan(120));
      expect(stats.onShiftDuration, const Duration(minutes: 5));
      expect(stats.pointCount, 2);
    });

    // Ordering moved OUT of this function and into `buildMergedSeries`, which
    // owns the contract's three-level sort. This function now consumes the
    // series in the order it is given — feeding it unsorted input is a caller
    // bug, not something to paper over with a second sort.
    test('consumes the given order without re-sorting', () {
      final ordered = computeTrajectoryStats([
        _pt(lat: 25.000, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
        _pt(lat: 25.001, lng: 121.0, occurredAtClient: '2026-05-15T09:05:00Z'),
        _pt(lat: 25.002, lng: 121.0, occurredAtClient: '2026-05-15T09:10:00Z'),
      ]);
      expect(ordered.onShiftDuration, const Duration(minutes: 10));
      expect(ordered.pointCount, 3);
    });

    test('duration is clamped to zero if timestamps regress', () {
      final stats = computeTrajectoryStats([
        _pt(lat: 25.0, lng: 121.0, occurredAtClient: '2026-05-15T09:10:00Z'),
        _pt(lat: 25.0, lng: 121.0, occurredAtClient: '2026-05-15T09:00:00Z'),
      ]);
      expect(stats.onShiftDuration, Duration.zero);
    });

    // The point of the merge: the clock-in→first-ping and last-ping→clock-out
    // legs used to be invisible to both stats.
    test('includes the head and tail legs contributed by checkin events', () {
      const clockIn = '2026-05-15T08:00:00Z';
      const firstPing = '2026-05-15T09:00:00Z';
      const lastPing = '2026-05-15T16:00:00Z';
      const clockOut = '2026-05-15T17:00:00Z';

      final pingsOnly = computeTrajectoryStats([
        _pt(lat: 25.001, lng: 121.0, occurredAtClient: firstPing),
        _pt(lat: 25.002, lng: 121.0, occurredAtClient: lastPing),
      ]);
      final merged = computeTrajectoryStats([
        _pt(
          lat: 25.000,
          lng: 121.0,
          occurredAtClient: clockIn,
          originRank: originEvent,
          id: 'in',
        ),
        _pt(lat: 25.001, lng: 121.0, occurredAtClient: firstPing),
        _pt(lat: 25.002, lng: 121.0, occurredAtClient: lastPing),
        _pt(
          lat: 25.003,
          lng: 121.0,
          occurredAtClient: clockOut,
          originRank: originEvent,
          id: 'out',
        ),
      ]);

      expect(merged.distanceMeters, greaterThan(pingsOnly.distanceMeters));
      // Duration becomes the real clock-in→clock-out span (9h), not the
      // first-ping→last-ping span (7h).
      expect(pingsOnly.onShiftDuration, const Duration(hours: 7));
      expect(merged.onShiftDuration, const Duration(hours: 9));
      expect(merged.pointCount, 4);
    });
  });
}
