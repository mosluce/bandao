import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/location_ping.dart';
import '../../checkin/data/checkin_repository.dart';
import '../data/my_locations_repository.dart';
import '../data/trajectory_merge.dart';
import '../data/trajectory_stats.dart';

/// State held by `trajectoryProvider`.
class TrajectoryDayState {
  const TrajectoryDayState({
    required this.selectedDate,
    required this.pings,
    required this.stats,
    this.events = const [],
    this.mergedSeries = const [],
  });

  /// Calendar day in *local* time. The repository call converts to UTC
  /// before sending; the picker UI works in the user's wall clock.
  final DateTime selectedDate;
  final List<LocationPingDto> pings;
  final TrajectoryStats stats;

  /// The day's check-in events (clock in/out, transfer in/out), drawn as
  /// event-type markers. Markers come from this full list — including events
  /// the merge contract excludes from the line.
  final List<CheckinEventDto> events;

  /// Polyline vertices: pings plus genuine checkin coordinates, in contract
  /// order. See [buildMergedSeries].
  final List<MergedPoint> mergedSeries;

  TrajectoryDayState copyWith({
    DateTime? selectedDate,
    List<LocationPingDto>? pings,
    TrajectoryStats? stats,
    List<CheckinEventDto>? events,
    List<MergedPoint>? mergedSeries,
  }) {
    return TrajectoryDayState(
      selectedDate: selectedDate ?? this.selectedDate,
      pings: pings ?? this.pings,
      stats: stats ?? this.stats,
      events: events ?? this.events,
      mergedSeries: mergedSeries ?? this.mergedSeries,
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
    final current =
        state.valueOrNull?.selectedDate ?? _startOfDay(DateTime.now());
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() => _fetchFor(current));
  }

  Future<TrajectoryDayState> _fetchFor(DateTime startOfDay) async {
    final repo = await ref.read(myLocationsRepositoryProvider.future);
    final next = startOfDay.add(const Duration(days: 1));
    final pings = await repo.listForRange(from: startOfDay, to: next);
    final events = await _fetchDayEvents(startOfDay, next);
    final merged = buildMergedSeries(pings, events);
    return TrajectoryDayState(
      selectedDate: startOfDay,
      pings: pings,
      stats: computeTrajectoryStats(merged),
      events: events,
      mergedSeries: merged,
    );
  }

  /// Fetch the day's check-in events — both the markers and, via the merge
  /// contract, the line's endpoints. Range-scoped server-side: over-fetching a
  /// newest-first page and filtering here returned nothing once the day fell
  /// outside the page's reach, which silently dropped the day's events
  /// entirely.
  ///
  /// Best-effort: any failure degrades to an empty list so the ping path still
  /// renders.
  Future<List<CheckinEventDto>> _fetchDayEvents(
    DateTime startOfDay,
    DateTime next,
  ) async {
    try {
      final checkin = await ref.read(checkinRepositoryProvider.future);
      // 200 is the endpoint's server-side cap; one day never approaches it.
      return await checkin.events(limit: 200, from: startOfDay, to: next);
    } catch (_) {
      return const [];
    }
  }

  static DateTime _startOfDay(DateTime t) => DateTime(t.year, t.month, t.day);
}

final trajectoryProvider =
    AsyncNotifierProvider<TrajectoryController, TrajectoryDayState>(
  TrajectoryController.new,
);
