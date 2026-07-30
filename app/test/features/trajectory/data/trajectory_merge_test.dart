import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/trajectory/data/trajectory_merge.dart';

// These fixtures mirror the admin-web suite in
// `admin-web/test/utils/trajectoryMerge.test.ts`. Both platforms must agree on
// every case below — that agreement IS the contract.

LocationPingDto _ping(
  String id,
  String iso, {
  double lat = 25,
  double lng = 121,
}) {
  return LocationPingDto(
    id: id,
    appUserId: 'u1',
    lat: lat,
    lng: lng,
    occurredAtClient: iso,
    occurredAtServer: iso,
  );
}

CheckinEventDto _event(
  String id,
  String iso, {
  double lat = 25,
  double lng = 121,
  EventSource source = EventSource.app,
  CheckinEventType eventType = CheckinEventType.clockIn,
}) {
  return CheckinEventDto(
    id: id,
    appUserId: 'u1',
    eventType: eventType,
    occurredAtClient: iso,
    occurredAtServer: iso,
    source: source,
    initiatedByKind: source == EventSource.adminForce
        ? EventInitiatorKind.dashboardUser
        : EventInitiatorKind.appUser,
    initiatedById: 'x1',
    location: EventLocation(coordinates: GeoPoint(lat: lat, lng: lng)),
    hasSkewWarning: false,
  );
}

void main() {
  group('buildMergedSeries', () {
    test('interleaves pings and events by time', () {
      final merged = buildMergedSeries(
        [
          _ping('p1', '2026-03-01T08:04:00Z'),
          _ping('p2', '2026-03-01T08:12:00Z'),
          _ping('p3', '2026-03-01T12:33:00Z'),
          _ping('p4', '2026-03-01T17:52:00Z'),
        ],
        [
          _event('e1', '2026-03-01T08:00:00Z'),
          _event('e2', '2026-03-01T12:30:00Z'),
          _event('e3', '2026-03-01T17:55:00Z'),
        ],
      );

      expect(
        merged.map((p) => p.id).toList(),
        ['e1', 'p1', 'p2', 'e2', 'p3', 'p4', 'e3'],
      );
    });

    test('puts the clock-in first and the clock-out last', () {
      final merged = buildMergedSeries(
        [_ping('p1', '2026-03-01T09:00:00Z', lat: 25.1, lng: 121.1)],
        [
          _event('in', '2026-03-01T08:00:00Z', lat: 25.0, lng: 121.0),
          _event(
            'out',
            '2026-03-01T17:00:00Z',
            lat: 25.9,
            lng: 121.9,
            eventType: CheckinEventType.clockOut,
          ),
        ],
      );

      expect(merged.first.id, 'in');
      expect(merged.first.lat, 25.0);
      expect(merged.last.id, 'out');
      expect(merged.last.lat, 25.9);
    });

    test('excludes admin_force events (location is copied, not captured)', () {
      final merged = buildMergedSeries(
        [_ping('p1', '2026-03-01T09:00:00Z')],
        [
          _event('in', '2026-03-01T08:00:00Z'),
          _event(
            'forced',
            '2026-03-01T23:00:00Z',
            source: EventSource.adminForce,
            eventType: CheckinEventType.clockOut,
          ),
        ],
      );

      expect(merged.map((p) => p.id).toList(), ['in', 'p1']);
    });

    test('includes legacy_backfill events (coordinates are genuine)', () {
      final merged = buildMergedSeries(
        const [],
        [
          _event(
            'l1',
            '2025-08-01T08:00:00Z',
            source: EventSource.legacyBackfill,
          ),
          _event(
            'l2',
            '2025-08-01T17:00:00Z',
            source: EventSource.legacyBackfill,
            eventType: CheckinEventType.clockOut,
          ),
        ],
      );

      expect(merged.map((p) => p.id).toList(), ['l1', 'l2']);
    });

    test('breaks an equal-timestamp tie toward the event', () {
      final merged = buildMergedSeries(
        [_ping('p1', '2026-03-01T08:00:00Z')],
        [_event('e1', '2026-03-01T08:00:00Z')],
      );

      expect(merged.map((p) => p.id).toList(), ['e1', 'p1']);
      expect(merged.first.originRank, originEvent);
      expect(merged.last.originRank, originPing);
    });

    test('breaks a same-instant same-origin tie by id, making the order total',
        () {
      final merged = buildMergedSeries(
        [
          _ping('pb', '2026-03-01T08:00:00Z'),
          _ping('pa', '2026-03-01T08:00:00Z'),
        ],
        const [],
      );

      expect(merged.map((p) => p.id).toList(), ['pa', 'pb']);
    });

    test('orders by instant, not by raw string, across mixed offsets', () {
      // 08:30+08:00 === 00:30Z, which is EARLIER than 01:00Z even though the
      // raw string sorts later. A lexical compare would invert these.
      final merged = buildMergedSeries(
        [_ping('pingAt0100Z', '2026-03-01T01:00:00Z')],
        [_event('eventAt0830Plus8', '2026-03-01T08:30:00+08:00')],
      );

      expect(
        merged.map((p) => p.id).toList(),
        ['eventAt0830Plus8', 'pingAt0100Z'],
      );
    });

    test('keeps near-coincident points rather than deduplicating', () {
      final merged = buildMergedSeries(
        [_ping('p1', '2026-03-01T17:54:57Z', lat: 25.0, lng: 121.0)],
        [
          _event(
            'out',
            '2026-03-01T17:55:00Z',
            lat: 25.00002,
            lng: 121.00002,
            eventType: CheckinEventType.clockOut,
          ),
        ],
      );

      expect(merged.length, 2);
      expect(merged.map((p) => p.id).toList(), ['p1', 'out']);
    });

    test('returns an empty series for no input', () {
      expect(buildMergedSeries(const [], const []), isEmpty);
    });

    test('handles pings only and events only', () {
      expect(
        buildMergedSeries([_ping('p1', '2026-03-01T08:00:00Z')], const [])
            .length,
        1,
      );
      expect(
        buildMergedSeries(const [], [_event('e1', '2026-03-01T08:00:00Z')])
            .length,
        1,
      );
    });
  });

  group('EventSource wire mapping', () {
    // `legacy_backfill` used to be absent from the enum, so `fromJson` threw
    // and blanked the whole events page for any AppUser with imported history.
    test('round-trips every variant the API can emit', () {
      for (final wire in ['app', 'admin_force', 'legacy_backfill']) {
        expect(EventSource.fromJson(wire).toJson(), wire);
      }
    });

    test('rejects an unknown variant', () {
      expect(() => EventSource.fromJson('nope'), throwsArgumentError);
    });
  });
}
