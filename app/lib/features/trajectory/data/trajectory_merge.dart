import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/location_ping.dart';

/// Shared trajectory point merge contract.
///
/// The polyline's vertices are the day's location pings PLUS the day's checkin
/// event coordinates, so the drawn line passes through the points where the
/// worker actually clocked in, transferred, and clocked out. Without the merge
/// those markers float off the path by construction: the tracker only starts
/// after the server confirms the clock-in, stops the moment 下班 is tapped, and
/// samples on a 100m AND 60s filter that never lands exactly on an event.
///
/// This is a contract shared verbatim with admin-web
/// (`admin-web/utils/trajectoryMerge.ts`) — the source filter, the three-level
/// ordering, and the instant-based comparison MUST match so both surfaces draw
/// the same line. See openspec capability `app-personal-trajectory`,
/// "Trajectory point merge contract".

/// Origin rank — an event sorts before a ping at the same instant.
const int originEvent = 0;
const int originPing = 1;

/// One vertex of the day's polyline.
class MergedPoint {
  const MergedPoint({
    required this.lat,
    required this.lng,
    required this.occurredAtClient,
    required this.originRank,
    required this.id,
  });

  final double lat;
  final double lng;

  /// Raw RFC3339 string as the server sent it. Kept unparsed so the
  /// time-of-day colour lookup reads the same field the contract orders by.
  final String occurredAtClient;

  /// [originEvent] or [originPing]; the second sort level.
  final int originRank;

  /// ObjectId hex; the final sort level, making the order total.
  final String id;

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is MergedPoint &&
          other.lat == lat &&
          other.lng == lng &&
          other.occurredAtClient == occurredAtClient &&
          other.originRank == originRank &&
          other.id == id;

  @override
  int get hashCode => Object.hash(lat, lng, occurredAtClient, originRank, id);

  @override
  String toString() => 'MergedPoint($id, $occurredAtClient, $lat, $lng)';
}

/// Build the ordered vertex list for one day.
///
/// Events with `source == EventSource.adminForce` are excluded: force-checkout
/// copies its location from the AppUser's *previous* event rather than
/// capturing a position, so a vertex there would draw a leg from wherever the
/// worker actually was back to a stale coordinate — the one case where merging
/// makes the picture actively wrong rather than merely coarse. `app` and
/// `legacyBackfill` both carry genuine captured coordinates and are included.
///
/// Excluding an event from the series does NOT exclude it from the markers;
/// callers keep drawing markers from the full event list.
List<MergedPoint> buildMergedSeries(
  List<LocationPingDto> pings,
  List<CheckinEventDto> events,
) {
  final points = <MergedPoint>[];

  for (final p in pings) {
    points.add(
      MergedPoint(
        lat: p.lat,
        lng: p.lng,
        occurredAtClient: p.occurredAtClient,
        originRank: originPing,
        id: p.id,
      ),
    );
  }

  for (final e in events) {
    if (e.source == EventSource.adminForce) continue;
    points.add(
      MergedPoint(
        lat: e.location.coordinates.lat,
        lng: e.location.coordinates.lng,
        occurredAtClient: e.occurredAtClient,
        originRank: originEvent,
        id: e.id,
      ),
    );
  }

  // Compare parsed instants, never raw strings. The pings and the events come
  // from two independent serialisation sites; both emit UTC today, but a
  // lexical compare would silently mis-order the merged series the moment one
  // of them emitted an offset like `+08:00` instead.
  points.sort((a, b) {
    final at = _instantMillis(a.occurredAtClient);
    final bt = _instantMillis(b.occurredAtClient);
    if (at != bt) return at.compareTo(bt);
    if (a.originRank != b.originRank) {
      return a.originRank.compareTo(b.originRank);
    }
    return a.id.compareTo(b.id);
  });

  return points;
}

/// Epoch millis of an RFC3339 string. An unparseable value sorts first rather
/// than throwing — a single malformed row must not blank the whole day's line.
int _instantMillis(String iso) =>
    DateTime.tryParse(iso)?.toUtc().millisecondsSinceEpoch ?? 0;
