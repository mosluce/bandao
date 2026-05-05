import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../../core/storage/privacy_url.dart';
import '../../../l10n/app_localizations.dart';

/// First-time consent gate for location tracking. Returns:
///   - `true` when the user taps `[同意並上班]`
///   - `false` when the user taps `[取消]` or dismisses
///
/// Caller is responsible for writing the per-AppUser consent flag on `true`
/// and calling the existing clock_in flow afterwards.
Future<bool> showLocationConsentDialog(
  BuildContext context,
  WidgetRef ref,
) async {
  final result = await showDialog<bool>(
    context: context,
    barrierDismissible: false,
    builder: (dialogContext) => _LocationConsentDialog(parentRef: ref),
  );
  return result == true;
}

class _LocationConsentDialog extends ConsumerWidget {
  const _LocationConsentDialog({required this.parentRef});

  final WidgetRef parentRef;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    return AlertDialog(
      title: Text(l10n.locationConsentTitle),
      content: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(l10n.locationConsentBody),
            const SizedBox(height: 12),
            _bullet(context, l10n.locationConsentBulletCadence),
            _bullet(context, l10n.locationConsentBulletDistance),
            _bullet(context, l10n.locationConsentBulletRetention),
            _bullet(context, l10n.locationConsentBulletAudience),
            const SizedBox(height: 12),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: () => _openPrivacy(context, parentRef),
                child: Text(l10n.locationConsentPrivacyLink),
              ),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.locationConsentCancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(l10n.locationConsentProceed),
        ),
      ],
    );
  }

  Widget _bullet(BuildContext context, String text) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 2),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          const Text('•  '),
          Expanded(child: Text(text)),
        ],
      ),
    );
  }

  Future<void> _openPrivacy(BuildContext context, WidgetRef ref) async {
    final url = await ref.read(effectivePrivacyUrlProvider.future);
    final uri = Uri.parse(url);
    final ok = await launchUrl(uri, mode: LaunchMode.inAppBrowserView);
    if (!ok && context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(AppLocalizations.of(context).errorGeneric)),
      );
    }
  }
}
