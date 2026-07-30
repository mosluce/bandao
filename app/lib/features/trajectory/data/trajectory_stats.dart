import 'package:latlong2/latlong.dart';

import 'trajectory_merge.dart';

/// Distilled numbers for the trajectory screen + home summary card.
class TrajectoryStats {
  const TrajectoryStats({
    required this.distanceMeters,
    required this.onShiftDuration,
    required this.pointCount,
  });

  final double distanceMeters;
  final Duration onShiftDuration;

  /// Number of plotted points — the merged series' length, so the 位置點
  /// figure matches what is actually drawn on the map.
  final int pointCount;

  static const empty = TrajectoryStats(
    distanceMeters: 0,
    onShiftDuration: Duration.zero,
    pointCount: 0,
  );
}

/// Compute distance + duration from one day's merged series.
///
/// - Distance: sum of geodesic distances between consecutive points. Uses
///   `latlong2`'s `Distance().distance()` (Vincenty by default, falls back to
///   Haversine). Because the series carries the day's checkin coordinates, this
///   now includes the leg from the clock-in point to the first ping and from
///   the last ping to the clock-out point — both previously missing.
/// - On-shift duration: span between the first and last merged point. On a
///   normal day that IS the clock-in→clock-out span, so it no longer
///   under-reports by the tracker's start-up delay and its stop-on-tap.
///
/// The caller supplies a series already ordered by [buildMergedSeries]; this
/// function does not reorder it.
TrajectoryStats computeTrajectoryStats(List<MergedPoint> points) {
  if (points.isEmpty) {
    return TrajectoryStats.empty;
  }

  double meters = 0;
  if (points.length > 1) {
    const distance = Distance();
    for (var i = 1; i < points.length; i++) {
      meters += distance.distance(
        LatLng(points[i - 1].lat, points[i - 1].lng),
        LatLng(points[i].lat, points[i].lng),
      );
    }
  }

  final firstTs = DateTime.tryParse(points.first.occurredAtClient);
  final lastTs = DateTime.tryParse(points.last.occurredAtClient);
  final dur = (firstTs != null && lastTs != null)
      ? lastTs.difference(firstTs)
      : Duration.zero;

  return TrajectoryStats(
    distanceMeters: meters,
    onShiftDuration: dur.isNegative ? Duration.zero : dur,
    pointCount: points.length,
  );
}
