import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_status.dart';
import '../../../l10n/app_localizations.dart';
import '../state/checkin_status_provider.dart';
import '../state/effective_status_provider.dart';

class CheckinStatusPill extends ConsumerWidget {
  const CheckinStatusPill({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    final eff = ref.watch(effectiveStatusProvider);
    final serverStatus = ref.watch(checkinStatusProvider).valueOrNull;

    final (icon, label, fg) = _styleFor(eff.status, scheme, l10n);
    final region = eff.status == AppUserCheckinStatus.offDuty
        ? null
        : _regionLabel(serverStatus, l10n);
    final elapsed = _elapsedLabel(eff, l10n);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 14),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: scheme.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(icon, color: fg),
              const SizedBox(width: 8),
              Text(
                label,
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
              ),
            ],
          ),
          if (region != null) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              region,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
            ),
          ],
          if (elapsed != null) ...<Widget>[
            const SizedBox(height: 4),
            Text(
              elapsed,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
            ),
          ],
        ],
      ),
    );
  }

  (IconData, String, Color) _styleFor(
    AppUserCheckinStatus status,
    ColorScheme scheme,
    AppLocalizations l10n,
  ) {
    switch (status) {
      case AppUserCheckinStatus.offDuty:
        return (Icons.bed_outlined, l10n.statusOffDuty, scheme.outline);
      case AppUserCheckinStatus.onSite:
        return (Icons.work_history, l10n.statusOnSite, scheme.primary);
      case AppUserCheckinStatus.inTransit:
        return (Icons.directions_walk, l10n.statusInTransit, scheme.tertiary);
    }
  }

  String? _regionLabel(
    CheckinUserStatusDto? server,
    AppLocalizations l10n,
  ) {
    final last = server?.lastEvent;
    if (last == null) return null;
    final region = last.location.regionName;
    if (region != null && region.isNotEmpty) return region;
    final lat = last.location.coordinates.lat.toStringAsFixed(4);
    final lng = last.location.coordinates.lng.toStringAsFixed(4);
    return '$lat, $lng';
  }

  String? _elapsedLabel(EffectiveStatus eff, AppLocalizations l10n) {
    if (eff.status == AppUserCheckinStatus.offDuty) return null;
    final start = eff.currentShiftStartedAt;
    if (start == null) return null;
    final startTime = DateTime.tryParse(start);
    if (startTime == null) return null;
    final delta = DateTime.now().difference(startTime);
    if (delta.isNegative) return null;
    final hours = delta.inHours;
    final minutes = delta.inMinutes.remainder(60);
    return l10n.elapsedShift(hours, minutes);
  }
}
