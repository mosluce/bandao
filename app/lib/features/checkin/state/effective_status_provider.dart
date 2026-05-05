import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/checkin_status.dart';
import '../data/checkin_queue_db.dart';
import 'checkin_queue_provider.dart';
import 'checkin_status_provider.dart';

class EffectiveStatus {
  const EffectiveStatus({
    required this.status,
    this.currentShiftStartedAt,
    required this.hasPendingTransition,
  });

  final AppUserCheckinStatus status;
  final String? currentShiftStartedAt;
  final bool hasPendingTransition;

  static const EffectiveStatus offDuty = EffectiveStatus(
    status: AppUserCheckinStatus.offDuty,
    hasPendingTransition: false,
  );

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is EffectiveStatus &&
        other.status == status &&
        other.currentShiftStartedAt == currentShiftStartedAt &&
        other.hasPendingTransition == hasPendingTransition;
  }

  @override
  int get hashCode =>
      Object.hash(status, currentShiftStartedAt, hasPendingTransition);
}

/// Pure state-machine transition; mirrors the server's
/// `apply_transition` in `api/src/domain/checkin.rs`. Returns null when the
/// transition is illegal — the caller treats that as "no change".
AppUserCheckinStatus? applyTransition(
  AppUserCheckinStatus from,
  CheckinEventType event,
) {
  switch (from) {
    case AppUserCheckinStatus.offDuty:
      if (event == CheckinEventType.clockIn) return AppUserCheckinStatus.onSite;
      return null;
    case AppUserCheckinStatus.onSite:
      if (event == CheckinEventType.clockOut) {
        return AppUserCheckinStatus.offDuty;
      }
      if (event == CheckinEventType.transferOut) {
        return AppUserCheckinStatus.inTransit;
      }
      return null;
    case AppUserCheckinStatus.inTransit:
      if (event == CheckinEventType.transferIn) {
        return AppUserCheckinStatus.onSite;
      }
      if (event == CheckinEventType.clockOut) {
        return AppUserCheckinStatus.offDuty;
      }
      return null;
  }
}

/// Pure reducer: replay non-`failed` queue rows over the server-confirmed
/// status. `pending` and `sending` rows contribute (optimistic); `failed`
/// rows are excluded so the status rolls back when the server rejects.
EffectiveStatus reduceEffectiveStatus({
  required CheckinUserStatusDto? serverStatus,
  required List<QueueRow> queue,
}) {
  final base = serverStatus?.status ?? AppUserCheckinStatus.offDuty;
  String? shiftStart = serverStatus?.currentShiftStartedAt;

  final rows = [...queue]
    ..sort((a, b) => a.occurredAtClient.compareTo(b.occurredAtClient));

  bool hasPending = false;
  AppUserCheckinStatus current = base;

  for (final row in rows) {
    if (row.status == 'failed') continue;
    if (row.status == 'pending' || row.status == 'sending') {
      hasPending = true;
    }
    final eventType = CheckinEventType.fromJson(row.eventType);
    final next = applyTransition(current, eventType);
    if (next == null) continue;
    if (current == AppUserCheckinStatus.offDuty &&
        next == AppUserCheckinStatus.onSite) {
      shiftStart = row.occurredAtClient;
    } else if (next == AppUserCheckinStatus.offDuty) {
      shiftStart = null;
    }
    current = next;
  }

  return EffectiveStatus(
    status: current,
    currentShiftStartedAt: shiftStart,
    hasPendingTransition: hasPending,
  );
}

/// Riverpod-exposed effective status combining server + local queue.
final effectiveStatusProvider = Provider<EffectiveStatus>((ref) {
  final server = ref.watch(checkinStatusProvider).valueOrNull;
  final queue = ref.watch(checkinQueueProvider).valueOrNull ?? const [];
  return reduceEffectiveStatus(serverStatus: server, queue: queue);
});
