import 'package:latlong2/latlong.dart';

import '../../../core/api/models/location_ping.dart';

/// Distilled numbers for the trajectory screen + home summary card.
class TrajectoryStats {
  const TrajectoryStats({
    required this.distanceMeters,
    required this.onShiftDuration,
    required this.pingCount,
  });

  final double distanceMeters;
  final Duration onShiftDuration;
  final int pingCount;

  static const empty = TrajectoryStats(
    distanceMeters: 0,
    onShiftDuration: Duration.zero,
    pingCount: 0,
  );
}

/// Compute distance + duration from a list of pings for one day.
///
/// - Distance: sum of geodesic distances between consecutive points after
///   sorting by `occurred_at_client` ascending. Uses `latlong2`'s
///   `Distance().distance()` (Vincenty by default, falls back to Haversine).
/// - On-shift duration: span between the earliest and latest ping. This is
///   an approximation — a precise "in-shift" answer would need to cross
///   with `checkin_events` which is out of scope for this surface.
TrajectoryStats computeTrajectoryStats(List<LocationPingDto> pings) {
  if (pings.isEmpty) {
    return TrajectoryStats.empty;
  }
  final sorted = [...pings]
    ..sort((a, b) => a.occurredAtClient.compareTo(b.occurredAtClient));

  double meters = 0;
  if (sorted.length > 1) {
    const distance = Distance();
    for (var i = 1; i < sorted.length; i++) {
      meters += distance.distance(
        LatLng(sorted[i - 1].lat, sorted[i - 1].lng),
        LatLng(sorted[i].lat, sorted[i].lng),
      );
    }
  }

  final firstTs = DateTime.parse(sorted.first.occurredAtClient);
  final lastTs = DateTime.parse(sorted.last.occurredAtClient);
  final dur = lastTs.difference(firstTs);

  return TrajectoryStats(
    distanceMeters: meters,
    onShiftDuration: dur.isNegative ? Duration.zero : dur,
    pingCount: sorted.length,
  );
}
