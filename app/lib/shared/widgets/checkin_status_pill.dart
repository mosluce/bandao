import 'package:flutter/material.dart';

import '../../l10n/app_localizations.dart';

/// Placeholder for the future checkin status pill. `add-app-checkin` swaps
/// this for the real version. Kept in `shared/widgets/` so the replacement is
/// a single-file move rather than a search-and-edit across the home screen.
class CheckinStatusPill extends StatelessWidget {
  const CheckinStatusPill({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
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
              Icon(Icons.hourglass_empty, color: scheme.onSurfaceVariant),
              const SizedBox(width: 8),
              Text(
                l10n.homeStubTitle,
                style: Theme.of(context).textTheme.titleMedium,
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            l10n.homeStubSubtitle,
            style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
          ),
        ],
      ),
    );
  }
}
