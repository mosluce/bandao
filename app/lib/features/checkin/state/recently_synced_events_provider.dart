import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_event.dart';

/// In-memory cache of events that the queue processor just successfully
/// submitted. The history screen reads this so a just-synced row stays
/// visible after the corresponding `pending_events` row is deleted on 2xx.
///
/// Bounded at 50 entries — when the user paginates server history with
/// `[載入更多]`, the events also start coming through `_serverEvents` and the
/// merge dedupes by `id`. Cap drops the oldest on push beyond 50; in
/// pathological cases (50+ events without ever opening history) the dropped
/// row is still on the server and the next manual refresh fetches it.
class RecentlySyncedEventsNotifier extends Notifier<List<CheckinEventDto>> {
  static const int cap = 50;

  @override
  List<CheckinEventDto> build() => const <CheckinEventDto>[];

  void push(CheckinEventDto event) {
    final next = <CheckinEventDto>[event, ...state];
    if (next.length > cap) {
      next.removeRange(cap, next.length);
    }
    state = next;
  }

  void clear() {
    state = const <CheckinEventDto>[];
  }
}

final recentlySyncedEventsProvider =
    NotifierProvider<RecentlySyncedEventsNotifier, List<CheckinEventDto>>(
  RecentlySyncedEventsNotifier.new,
);
