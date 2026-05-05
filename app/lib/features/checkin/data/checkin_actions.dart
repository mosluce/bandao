import 'dart:async';

import 'package:drift/drift.dart' show Value;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/submit_checkin_event.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../state/location_permission_provider.dart';
import 'background_sync.dart';
import 'checkin_queue_db.dart';
import 'geolocation_service.dart';
import 'queue_processor.dart';

enum EnqueueOutcome {
  enqueued,
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
      occurredAtClient: Value(nowOccurredAtClient(now)),
      enqueuedAt: Value(now.toIso8601String()),
    ),);

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
