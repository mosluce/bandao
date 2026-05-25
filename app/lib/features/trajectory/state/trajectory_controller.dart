import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/location_ping.dart';
import '../data/my_locations_repository.dart';
import '../data/trajectory_stats.dart';

/// State held by `trajectoryProvider`.
class TrajectoryDayState {
  const TrajectoryDayState({
    required this.selectedDate,
    required this.pings,
    required this.stats,
  });

  /// Calendar day in *local* time. The repository call converts to UTC
  /// before sending; the picker UI works in the user's wall clock.
  final DateTime selectedDate;
  final List<LocationPingDto> pings;
  final TrajectoryStats stats;

  TrajectoryDayState copyWith({
    DateTime? selectedDate,
    List<LocationPingDto>? pings,
    TrajectoryStats? stats,
  }) {
    return TrajectoryDayState(
      selectedDate: selectedDate ?? this.selectedDate,
      pings: pings ?? this.pings,
      stats: stats ?? this.stats,
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
    return TrajectoryDayState(
      selectedDate: startOfDay,
      pings: pings,
      stats: computeTrajectoryStats(pings),
    );
  }

  static DateTime _startOfDay(DateTime t) =>
      DateTime(t.year, t.month, t.day);
}

final trajectoryProvider =
    AsyncNotifierProvider<TrajectoryController, TrajectoryDayState>(
  TrajectoryController.new,
);
