import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';

import '../../../l10n/app_localizations.dart';
import '../data/geolocation_service.dart';
import '../state/location_permission_provider.dart';

/// Inline banner shown above the home buttons when location permission is
/// denied or denied-forever. Hidden when permission is `granted`,
/// `whileInUse`, `always`, or `notDetermined` (the prompt fires on first
/// tap).
class LocationPermissionBlocker extends ConsumerWidget {
  const LocationPermissionBlocker({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final permission = ref.watch(locationPermissionProvider).valueOrNull;
    // Only show the blocker for `deniedForever` — that's the state where the
    // OS won't re-prompt and Settings is the only path forward. `denied`
    // covers the "never asked" first-install case; we let the button itself
    // trigger the prompt on first tap.
    if (permission != LocationPermission.deniedForever) {
      return const SizedBox.shrink();
    }
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.errorContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: <Widget>[
          Icon(Icons.location_disabled, color: scheme.onErrorContainer),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              l10n.locationBlockerMessage,
              style: TextStyle(color: scheme.onErrorContainer),
            ),
          ),
          const SizedBox(width: 8),
          TextButton(
            onPressed: () async {
              await ref.read(geolocationServiceProvider).openSettings();
              await ref
                  .read(locationPermissionProvider.notifier)
                  .refresh();
            },
            child: Text(l10n.locationBlockerOpenSettings),
          ),
        ],
      ),
    );
  }
}
