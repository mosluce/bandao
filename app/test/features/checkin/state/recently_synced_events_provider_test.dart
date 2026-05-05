import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/models/checkin_event.dart';
import 'package:argus_app/features/checkin/state/recently_synced_events_provider.dart';

void main() {
  ProviderContainer makeContainer() {
    final container = ProviderContainer();
    addTearDown(container.dispose);
    return container;
  }

  test('starts empty', () {
    final container = makeContainer();
    expect(container.read(recentlySyncedEventsProvider), isEmpty);
  });

  test('push adds events newest-first', () {
    final container = makeContainer();
    final notifier = container.read(recentlySyncedEventsProvider.notifier);
    notifier.push(_event('a', '2026-05-04T08:00:00+08:00'));
    notifier.push(_event('b', '2026-05-04T09:00:00+08:00'));

    final list = container.read(recentlySyncedEventsProvider);
    expect(list.map((e) => e.id), ['b', 'a']);
  });

  test('cap drops oldest beyond 50 entries', () {
    final container = makeContainer();
    final notifier = container.read(recentlySyncedEventsProvider.notifier);
    for (var i = 0; i < 55; i++) {
      notifier.push(_event('e$i', '2026-05-04T08:00:00+08:00'));
    }
    final list = container.read(recentlySyncedEventsProvider);
    expect(list.length, RecentlySyncedEventsNotifier.cap);
    // Newest pushed last is at the head; oldest 5 dropped.
    expect(list.first.id, 'e54');
    expect(list.any((e) => e.id == 'e0'), isFalse);
    expect(list.any((e) => e.id == 'e4'), isFalse);
    expect(list.any((e) => e.id == 'e5'), isTrue);
  });

  test('clear empties the list', () {
    final container = makeContainer();
    final notifier = container.read(recentlySyncedEventsProvider.notifier);
    notifier.push(_event('a', '2026-05-04T08:00:00+08:00'));
    notifier.push(_event('b', '2026-05-04T09:00:00+08:00'));
    notifier.clear();
    expect(container.read(recentlySyncedEventsProvider), isEmpty);
  });
}

CheckinEventDto _event(String id, String t) => CheckinEventDto(
      id: id,
      appUserId: 'u1',
      eventType: CheckinEventType.clockIn,
      occurredAtClient: t,
      occurredAtServer: '2026-05-04T00:00:00Z',
      source: EventSource.app,
      initiatedByKind: EventInitiatorKind.appUser,
      initiatedById: 'u1',
      location:
          const EventLocation(coordinates: GeoPoint(lat: 25, lng: 121)),
      hasSkewWarning: false,
    );
