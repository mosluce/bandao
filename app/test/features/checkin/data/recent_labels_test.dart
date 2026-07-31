import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/features/checkin/data/recent_labels.dart';

final _now = DateTime.parse('2026-07-31T09:00:00Z');

CheckinEventDto _ev(String? label, {Duration ago = Duration.zero}) {
  final at = _now.subtract(ago).toIso8601String();
  return CheckinEventDto(
    id: 'e${label ?? 'null'}${ago.inMinutes}',
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

void main() {
  group('computeRecentLabels', () {
    test('orders by frequency, not recency', () {
      // 乙工地 is the most recent but 甲工地 is used more often. Frequency wins
      // so the dominant site keeps a stable position and becomes muscle
      // memory — the entire point of the suggestions.
      final labels = computeRecentLabels(
        [
          _ev('乙工地', ago: const Duration(minutes: 1)),
          _ev('甲工地', ago: const Duration(hours: 2)),
          _ev('甲工地', ago: const Duration(hours: 3)),
          _ev('甲工地', ago: const Duration(hours: 4)),
        ],
        now: _now,
      );

      expect(labels, ['甲工地', '乙工地']);
    });

    test('caps the list', () {
      final events = <CheckinEventDto>[
        for (var i = 0; i < 19; i++)
          _ev('site$i', ago: Duration(hours: i + 1)),
      ];

      expect(computeRecentLabels(events, now: _now), hasLength(6));
    });

    test('excludes events outside the window', () {
      final labels = computeRecentLabels(
        [
          _ev('近期', ago: const Duration(days: 29)),
          _ev('過期', ago: const Duration(days: 31)),
        ],
        now: _now,
      );

      expect(labels, ['近期']);
    });

    test('skips events with no label', () {
      // Every event the app has created so far has a null label, so on a
      // freshly-migrated Org the imported history is the only source.
      final labels = computeRecentLabels(
        [_ev(null), _ev(''), _ev('   '), _ev('甲工地')],
        now: _now,
      );

      expect(labels, ['甲工地']);
    });

    test('trims surrounding whitespace rather than treating it as distinct', () {
      final labels = computeRecentLabels(
        [_ev(' 甲工地 '), _ev('甲工地')],
        now: _now,
      );

      expect(labels, ['甲工地']);
    });

    test('returns nothing when there is no history', () {
      expect(computeRecentLabels(const [], now: _now), isEmpty);
    });
  });
}
