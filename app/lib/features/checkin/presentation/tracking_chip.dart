import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../l10n/app_localizations.dart';
import '../state/location_tracking_controller.dart';

/// Always-visible chip while the location tracker is active. Reads
/// `LocationTrackingController.isRunning` for visibility, `startedAt` for
/// the elapsed counter, and rebuilds on each per-second tick stream
/// emission.
class TrackingChip extends ConsumerWidget {
  const TrackingChip({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final isRunning = ref.watch(locationTrackingControllerProvider);
    if (!isRunning) return const SizedBox.shrink();

    final controller = ref.read(locationTrackingControllerProvider.notifier);
    final startedAt = controller.startedAt;
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;

    return StreamBuilder<DateTime>(
      stream: controller.tickStream,
      builder: (context, _) {
        final now = DateTime.now();
        final elapsed = startedAt == null
            ? Duration.zero
            : now.difference(startedAt);
        return Chip(
          avatar: Icon(Icons.my_location, color: scheme.primary, size: 18),
          label: Text('${l10n.trackingChipLabel} · ${_formatElapsed(elapsed)}'),
        );
      },
    );
  }

  String _formatElapsed(Duration d) {
    final hours = d.inHours;
    final minutes = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final seconds = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    if (hours > 0) {
      return '${hours.toString().padLeft(2, '0')}:$minutes:$seconds';
    }
    return '$minutes:$seconds';
  }
}
