import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_status.dart';
import '../../../core/storage/secure_storage.dart';
import '../../../l10n/app_localizations.dart';
import '../data/checkin_queue_db.dart';
import '../state/checkin_status_provider.dart';

/// One-shot banner that surfaces when the app boots and detects that the
/// previous session's tracker did not stop cleanly while a shift was in
/// progress. Triggers when:
///
///   • server status (from `checkinStatusProvider`) is non-`off_duty`, AND
///   • the secure-storage `last_clean_stop` flag is missing OR is older
///     than the most recently enqueued local ping.
///
/// Auto-dismisses after 10 seconds. Also dismissible by tap.
class TrackingRecoveryBanner extends ConsumerStatefulWidget {
  const TrackingRecoveryBanner({super.key});

  @override
  ConsumerState<TrackingRecoveryBanner> createState() =>
      _TrackingRecoveryBannerState();
}

class _TrackingRecoveryBannerState
    extends ConsumerState<TrackingRecoveryBanner> {
  bool _shouldShow = false;
  bool _checked = false;
  Timer? _autoDismiss;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _evaluate());
  }

  @override
  void dispose() {
    _autoDismiss?.cancel();
    super.dispose();
  }

  Future<void> _evaluate() async {
    if (_checked) return;
    _checked = true;
    CheckinUserStatusDto? status;
    try {
      status = await ref.read(checkinStatusProvider.future);
    } catch (_) {
      // Network / parse error during the bootstrap fetch — banner can't
      // make a recovery judgment without a status, so silently bail.
      return;
    }
    if (!mounted) return;
    if (status == null || status.status == AppUserCheckinStatus.offDuty) {
      return;
    }
    final storage = ref.read(secureStorageProvider);
    DateTime? lastClean;
    try {
      lastClean = await storage.readLocationTrackingLastCleanStop();
    } catch (_) {
      return;
    }
    if (!mounted) return;
    DateTime? latestEnqueued;
    try {
      final db = ref.read(checkinQueueDbProvider);
      latestEnqueued = await db.latestPendingLocationEnqueuedAt();
    } catch (_) {
      // drift not opened in tests / fresh installs — treat as no rows.
      latestEnqueued = null;
    }
    final cleanIsStale = lastClean == null ||
        (latestEnqueued != null && lastClean.isBefore(latestEnqueued));
    if (!cleanIsStale) return;
    if (!mounted) return;
    setState(() => _shouldShow = true);
    _autoDismiss = Timer(const Duration(seconds: 10), () {
      if (mounted) setState(() => _shouldShow = false);
    });
  }

  void _dismiss() {
    _autoDismiss?.cancel();
    setState(() => _shouldShow = false);
  }

  @override
  Widget build(BuildContext context) {
    if (!_shouldShow) return const SizedBox.shrink();
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    return Material(
      color: scheme.tertiaryContainer,
      child: SafeArea(
        bottom: false,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          child: Row(
            children: <Widget>[
              Icon(Icons.info_outline, color: scheme.onTertiaryContainer),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  l10n.trackingRecoveryBannerMessage,
                  style: TextStyle(color: scheme.onTertiaryContainer),
                ),
              ),
              TextButton(
                onPressed: _dismiss,
                child: Text(l10n.trackingRecoveryBannerDismiss),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
