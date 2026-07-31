import 'dart:async';

import 'package:drift/drift.dart' show Value;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/submit_checkin_event.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../state/checkin_label_provider.dart';
import '../state/location_permission_provider.dart';
import 'background_sync.dart';
import 'checkin_queue_db.dart';
import 'geolocation_service.dart';
import 'queue_processor.dart';

enum EnqueueOutcome {
  enqueued,

  /// The label was missing or too long. The action buttons gate on this, so
  /// it only fires if something bypassed them.
  labelMissing,
  permissionDenied,
  locationUnavailable,
  notAuthenticated,
}

/// Side-effecting workflow for "user tapped an action button":
///
/// 1. Check permission; request if `notDetermined`.
/// 2. If granted, capture GPS (10s timeout, fallback to last known).
/// 3. Insert a `pending_events` row.
/// 4. Wake the foreground processor + ask the OS to wake us in background.
class CheckinActions {
  CheckinActions(this._ref);

  final Ref _ref;

  Future<EnqueueOutcome> enqueueEvent(CheckinEventType eventType) async {
    final auth = _ref.read(authProvider).valueOrNull;
    if (auth is! AuthAuthenticated) return EnqueueOutcome.notAuthenticated;

    // The buttons are gated on a valid label, so reaching here without one
    // means the gate was bypassed. Refuse rather than silently submitting an
    // unlabelled event — the whole point is that every event carries one.
    final label = _ref.read(checkinLabelProvider).trim();
    if (label.isEmpty || label.runes.length > checkinLabelMaxLength) {
      return EnqueueOutcome.labelMissing;
    }

    final permNotifier = _ref.read(locationPermissionProvider.notifier);
    var permission = await permNotifier.refresh();
    if (permission == LocationPermission.denied) {
      permission = await permNotifier.request();
    }
    if (permission == LocationPermission.denied ||
        permission == LocationPermission.deniedForever) {
      return EnqueueOutcome.permissionDenied;
    }

    final svc = _ref.read(geolocationServiceProvider);
    final captured = await svc.capture();
    if (captured == null) {
      return EnqueueOutcome.locationUnavailable;
    }

    final db = _ref.read(checkinQueueDbProvider);
    final now = DateTime.now();
    await db.enqueue(PendingEventsCompanion(
      appUserId: Value(auth.user.id),
      eventType: Value(eventType.toJson()),
      lat: Value(captured.point.lat),
      lng: Value(captured.point.lng),
      accuracy: Value(captured.accuracyMeters),
      manualLabel: Value(label),
      occurredAtClient: Value(nowOccurredAtClient(now)),
      enqueuedAt: Value(now.toIso8601String()),
    ),);

    // Cleared so the next check-in is labelled deliberately rather than
    // inheriting this one.
    _ref.read(checkinLabelProvider.notifier).state = '';

    // Foreground tick — drift's watchAll stream also wakes the processor,
    // but calling tick directly avoids a 1-frame UI lag.
    unawaited(_ref.read(queueProcessorProvider).tick());
    // Background OS wake-up (Android schedules immediately on enqueue;
    // iOS is best-effort).
    unawaited(requestBackgroundDrain());
    return EnqueueOutcome.enqueued;
  }
}

final checkinActionsProvider = Provider<CheckinActions>((ref) {
  return CheckinActions(ref);
});
