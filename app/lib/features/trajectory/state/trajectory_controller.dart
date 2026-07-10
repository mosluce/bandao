import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/location_ping.dart';
import '../../checkin/data/checkin_repository.dart';
import '../data/my_locations_repository.dart';
import '../data/trajectory_stats.dart';

/// State held by `trajectoryProvider`.
class TrajectoryDayState {
  const TrajectoryDayState({
    required this.selectedDate,
    required this.pings,
    required this.stats,
    this.events = const [],
  });

  /// Calendar day in *local* time. The repository call converts to UTC
  /// before sending; the picker UI works in the user's wall clock.
  final DateTime selectedDate;
  final List<LocationPingDto> pings;
  final TrajectoryStats stats;

  /// The day's check-in events (clock in/out, transfer in/out), drawn as
  /// event-type markers. The first `clock_in` anchors the start of the day, so
  /// the map renders as soon as the user has clocked in — before any pings.
  final List<CheckinEventDto> events;

  TrajectoryDayState copyWith({
    DateTime? selectedDate,
    List<LocationPingDto>? pings,
    TrajectoryStats? stats,
    List<CheckinEventDto>? events,
  }) {
    return TrajectoryDayState(
      selectedDate: selectedDate ?? this.selectedDate,
      pings: pings ?? this.pings,
      stats: stats ?? this.stats,
      events: events ?? this.events,
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
    final events = await _fetchDayEvents(startOfDay, next);
    return TrajectoryDayState(
      selectedDate: startOfDay,
      pings: pings,
      stats: computeTrajectoryStats(pings),
      events: events,
    );
  }

  /// Fetch the day's check-in events (for event markers + the clock-in start
  /// anchor). Best-effort: any failure degrades to an empty list so the ping
  /// path still renders. The events endpoint is cursor-paginated (no from/to);
  /// one page comfortably covers the trajectory's 8-day range for typical use.
  Future<List<CheckinEventDto>> _fetchDayEvents(
    DateTime startOfDay,
    DateTime next,
  ) async {
    try {
      final checkin = await ref.read(checkinRepositoryProvider.future);
      final events = await checkin.events(limit: 100);
      return events.where((e) {
        final t = DateTime.tryParse(e.occurredAtClient)?.toLocal();
        return t != null && !t.isBefore(startOfDay) && t.isBefore(next);
      }).toList(growable: false);
    } catch (_) {
      return const [];
    }
  }

  static DateTime _startOfDay(DateTime t) =>
      DateTime(t.year, t.month, t.day);
}

final trajectoryProvider =
    AsyncNotifierProvider<TrajectoryController, TrajectoryDayState>(
  TrajectoryController.new,
);
