import 'dart:io' show Platform;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/storage/secure_storage.dart';
import '../../../l10n/app_localizations.dart';

/// One-shot card explaining iOS background-sync limitations. Shown only on
/// iOS, only when `home.background_sync_tip_seen == false`. Dismissing it
/// writes the flag and the card stays hidden forever.
class BackgroundSyncTip extends ConsumerStatefulWidget {
  const BackgroundSyncTip({super.key});

  @override
  ConsumerState<BackgroundSyncTip> createState() => _BackgroundSyncTipState();
}

class _BackgroundSyncTipState extends ConsumerState<BackgroundSyncTip> {
  bool _hidden = false;
  bool _checking = true;

  @override
  void initState() {
    super.initState();
    _check();
  }

  Future<void> _check() async {
    if (!Platform.isIOS) {
      setState(() {
        _hidden = true;
        _checking = false;
      });
      return;
    }
    final storage = ref.read(secureStorageProvider);
    final seen = await storage.readBackgroundSyncTipSeen();
    setState(() {
      _hidden = seen;
      _checking = false;
    });
  }

  Future<void> _dismiss() async {
    final storage = ref.read(secureStorageProvider);
    await storage.markBackgroundSyncTipSeen();
    setState(() => _hidden = true);
  }

  @override
  Widget build(BuildContext context) {
    if (_checking || _hidden) return const SizedBox.shrink();
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.secondaryContainer,
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(Icons.info_outline, color: scheme.onSecondaryContainer),
              const SizedBox(width: 8),
              Text(
                l10n.backgroundTipTitle,
                style: TextStyle(
                  color: scheme.onSecondaryContainer,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            l10n.backgroundTipBody,
            style: TextStyle(color: scheme.onSecondaryContainer),
          ),
          const SizedBox(height: 6),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton(
              onPressed: _dismiss,
              child: Text(l10n.backgroundTipDismiss),
            ),
          ),
        ],
      ),
    );
  }
}
