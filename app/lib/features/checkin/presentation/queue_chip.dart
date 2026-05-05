import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../../l10n/app_localizations.dart';
import '../state/checkin_queue_provider.dart';

class QueueChip extends ConsumerWidget {
  const QueueChip({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final rows = ref.watch(checkinQueueProvider).valueOrNull ?? const [];
    if (rows.isEmpty) return const SizedBox.shrink();

    final pending = rows.where((r) => r.status == 'pending').length;
    final sending = rows.where((r) => r.status == 'sending').length;
    final failed = rows.where((r) => r.status == 'failed').length;

    if (pending == 0 && sending == 0 && failed == 0) {
      return const SizedBox.shrink();
    }

    final segments = <String>[];
    if (sending > 0) {
      segments.add(l10n.queueChipSending);
    } else if (pending > 0) {
      segments.add(l10n.queueChipPending(pending));
    }
    if (failed > 0) {
      segments.add(l10n.queueChipFailed(failed));
    }
    final label = segments.join(' · ');

    final scheme = Theme.of(context).colorScheme;
    final hasFailed = failed > 0;
    return ActionChip(
      avatar: Icon(
        hasFailed ? Icons.error_outline : Icons.cloud_upload_outlined,
        color: hasFailed ? scheme.error : scheme.primary,
        size: 18,
      ),
      label: Text(label),
      onPressed: () => context.go('/history'),
    );
  }
}
