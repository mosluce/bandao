import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../../l10n/app_localizations.dart';
import '../../checkin/data/location_tracking_service.dart';
import '../../checkin/state/checkin_status_provider.dart';
import '../../../core/api/models/checkin_status.dart';
import '../state/trajectory_controller.dart';

/// Home-screen card showing the AppUser's own movement totals for today —
/// the soft entry point into `/trajectory`. Hidden when there's nothing
/// to say (off-shift AND zero pings).
class TodaySummaryCard extends ConsumerStatefulWidget {
  const TodaySummaryCard({super.key});

  @override
  ConsumerState<TodaySummaryCard> createState() => _TodaySummaryCardState();
}

class _TodaySummaryCardState extends ConsumerState<TodaySummaryCard>
    with WidgetsBindingObserver {
  StreamSubscription<DateTime>? _tickSub;
  DateTime _lastRefresh = DateTime.fromMillisecondsSinceEpoch(0);

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    final svc = ref.read(locationTrackingServiceProvider);
    _tickSub = svc.tickStream.listen((_) => _maybeRefresh());
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      _maybeRefresh(force: true);
    }
  }

  void _maybeRefresh({bool force = false}) {
    final now = DateTime.now();
    if (!force && now.difference(_lastRefresh) < const Duration(seconds: 60)) {
      return;
    }
    _lastRefresh = now;
    ref.read(trajectoryProvider.notifier).refresh();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    _tickSub?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final trajectory = ref.watch(trajectoryProvider);
    final checkinStatus = ref.watch(checkinStatusProvider).valueOrNull;
    final day = trajectory.valueOrNull;

    final isOnShift = checkinStatus?.status == AppUserCheckinStatus.onSite ||
        checkinStatus?.status == AppUserCheckinStatus.inTransit;
    final hasData = (day?.stats.pingCount ?? 0) > 0;
    if (!isOnShift && !hasData) {
      return const SizedBox.shrink();
    }
    if (day == null) {
      return const SizedBox.shrink();
    }

    final km = day.stats.distanceMeters / 1000;
    final h = day.stats.onShiftDuration.inHours;
    final m = day.stats.onShiftDuration.inMinutes % 60;

    return Card(
      key: const ValueKey('todaySummaryCard'),
      child: InkWell(
        onTap: () => context.go('/trajectory'),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Row(
                children: [
                  const Icon(Icons.place_outlined, size: 20),
                  const SizedBox(width: 6),
                  Text(
                    l10n.trajectoryTodayCardTitle,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceAround,
                children: [
                  _Stat(
                    label: l10n.trajectoryStatDistance,
                    value: l10n.trajectoryDistanceKm(km),
                  ),
                  _Stat(
                    label: l10n.trajectoryStatDuration,
                    value: l10n.trajectoryDurationHm(h, m),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerRight,
                child: Text(
                  '${l10n.trajectoryTodayCardCta} →',
                  style: Theme.of(context).textTheme.labelMedium,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({required this.label, required this.value});
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label, style: theme.textTheme.bodySmall),
        const SizedBox(height: 4),
        Text(value, style: theme.textTheme.titleLarge),
      ],
    );
  }
}
