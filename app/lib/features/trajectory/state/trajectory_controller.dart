import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/location_ping.dart';
import '../../checkin/data/checkin_repository.dart';
import '../data/my_locations_repository.dart';
import '../data/trajectory_stats.dart';

/// Start anchor for a day's trajectory: the location + time of the first
/// `clock_in` event. Lets the screen draw the start marker as soon as the user
/// has clocked in, without waiting for location pings to accumulate.
class TrajectoryStart {
  const TrajectoryStart({
    required this.lat,
    required this.lng,
    required this.time,
  });

  final double lat;
  final double lng;

  /// Local wall-clock time of the clock-in (drives the start marker color).
  final DateTime time;
}

/// State held by `trajectoryProvider`.
class TrajectoryDayState {
  const TrajectoryDayState({
    required this.selectedDate,
    required this.pings,
    required this.stats,
    this.start,
  });

  /// Calendar day in *local* time. The repository call converts to UTC
  /// before sending; the picker UI works in the user's wall clock.
  final DateTime selectedDate;
  final List<LocationPingDto> pings;
  final TrajectoryStats stats;

  /// The day's clock-in anchor, when the user clocked in that day. Null when
  /// there was no clock-in (or its location was unavailable).
  final TrajectoryStart? start;

  TrajectoryDayState copyWith({
    DateTime? selectedDate,
    List<LocationPingDto>? pings,
    TrajectoryStats? stats,
    TrajectoryStart? start,
  }) {
    return TrajectoryDayState(
      selectedDate: selectedDate ?? this.selectedDate,
      pings: pings ?? this.pings,
      stats: stats ?? this.stats,
      start: start ?? this.start,
    );
  }
}

/// Backs the `/trajectory` screen. Holds the currently-selected day and
/// fetches the AppUser's own pings for that day. Recomputes stats on
/// every successful fetch.
class TrajectoryController extends AsyncNotifier<TrajectoryDayState> {
  @override
  Future<TrajectoryDayState> build() async {
    final today = _startOfDay(DateTime.now());
    return _fetchFor(today);
  }

  /// Switch to a different calendar day and refetch.
  Future<void> selectDate(DateTime day) async {
    final start = _startOfDay(day);
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() => _fetchFor(start));
  }

  /// Refresh the currently-selected day (used by the screen's pull-to-refresh
  /// and by the home summary card's debounced ticker).
  Future<void> refresh() async {
    final current = state.valueOrNull?.selectedDate ?? _startOfDay(DateTime.now());
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() => _fetchFor(current));
  }

  Future<TrajectoryDayState> _fetchFor(DateTime startOfDay) async {
    final repo = await ref.read(myLocationsRepositoryProvider.future);
    final next = startOfDay.add(const Duration(days: 1));
    final pings = await repo.listForRange(from: startOfDay, to: next);
    final start = await _fetchStartAnchor(startOfDay);
    return TrajectoryDayState(
      selectedDate: startOfDay,
      pings: pings,
      stats: computeTrajectoryStats(pings),
      start: start,
    );
  }

  /// Fetch the day's first `clock_in` event to anchor the start marker.
  /// Best-effort: any failure degrades to `null` so the ping path still
  /// renders. The events endpoint is cursor-paginated (no from/to); one page
  /// comfortably covers the trajectory's 8-day date range for typical users.
  Future<TrajectoryStart?> _fetchStartAnchor(DateTime startOfDay) async {
    try {
      final checkin = await ref.read(checkinRepositoryProvider.future);
      final events = await checkin.events(limit: 100);
      final next = startOfDay.add(const Duration(days: 1));
      CheckinEventDto? firstClockIn;
      DateTime? firstClockInAt;
      for (final e in events) {
        if (e.eventType != CheckinEventType.clockIn) continue;
        final t = DateTime.tryParse(e.occurredAtClient);
        if (t == null) continue;
        final local = t.toLocal();
        if (local.isBefore(startOfDay) || !local.isBefore(next)) continue;
        if (firstClockInAt == null || local.isBefore(firstClockInAt)) {
          firstClockIn = e;
          firstClockInAt = local;
        }
      }
      if (firstClockIn == null || firstClockInAt == null) return null;
      return TrajectoryStart(
        lat: firstClockIn.location.coordinates.lat,
        lng: firstClockIn.location.coordinates.lng,
        time: firstClockInAt,
      );
    } catch (_) {
      return null;
    }
  }

  static DateTime _startOfDay(DateTime t) =>
      DateTime(t.year, t.month, t.day);
}

final trajectoryProvider =
    AsyncNotifierProvider<TrajectoryController, TrajectoryDayState>(
  TrajectoryController.new,
);
