import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logger/logger.dart';

import '../../../core/api/api_error.dart';
import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/checkin_status.dart';
import '../../../core/api/models/submit_checkin_event.dart';
import '../../auth/state/auth_provider.dart';
import '../state/checkin_status_provider.dart';
import '../state/connectivity_provider.dart';
import '../state/recently_synced_events_provider.dart';
import 'checkin_queue_db.dart';
import 'checkin_repository.dart';

/// Owns the strict-serialization queue drain for `pending_events`.
///
/// `tick()` is re-entrant-safe — the in-flight `_running` guard plus the
/// `sending`-row check in the database are the locking mechanism.
/// The processor itself stays stateless beyond `_running`; persistent state
/// (attempts count, last error, last attempt time) lives on the row.
class QueueProcessor {
  QueueProcessor({
    required CheckinQueueDb db,
    required Future<CheckinRepository> Function() repo,
    required bool Function() isOnline,
    required Future<void> Function() onAuthExpired,
    void Function(CheckinUserStatusDto)? onStatusFresh,
    void Function(CheckinEventDto)? onEventSynced,
    DateTime Function() now = _defaultNow,
  })  : _db = db,
        _repo = repo,
        _isOnline = isOnline,
        _onAuthExpired = onAuthExpired,
        _onStatusFresh = onStatusFresh,
        _onEventSynced = onEventSynced,
        _now = now;

  final CheckinQueueDb _db;
  final Future<CheckinRepository> Function() _repo;
  final bool Function() _isOnline;
  final Future<void> Function() _onAuthExpired;
  final void Function(CheckinUserStatusDto)? _onStatusFresh;
  final void Function(CheckinEventDto)? _onEventSynced;
  final DateTime Function() _now;
  final Logger _log = Logger();

  bool _running = false;

  static const List<int> _backoffSeconds = [1, 2, 4, 8, 16, 30];

  /// Backoff schedule, exposed for unit tests. `attempts == 1` → 1s, ...,
  /// `attempts >= 6` → 30s. `attempts == 0` is meaningless (no attempt yet)
  /// but for safety returns the first step.
  static Duration nextDelay(int attempts) {
    final idx = (attempts - 1).clamp(0, _backoffSeconds.length - 1);
    return Duration(seconds: _backoffSeconds[idx]);
  }

  Future<void> tick() async {
    if (_running) return;
    _running = true;
    try {
      await _drainOnce();
    } finally {
      _running = false;
    }
  }

  Future<void> _drainOnce() async {
    // Skip while offline — don't burn attempts during a long offline stretch.
    if (!_isOnline()) return;

    // Single in-flight constraint.
    final inFlight = await _db.findInFlight();
    if (inFlight != null) return;

    final next = await _db.pickOldestPending();
    if (next == null) return;

    // Honor backoff: if we recently tried this row, wait it out.
    if (next.lastAttemptAt != null && next.attempts > 0) {
      final lastAt = DateTime.tryParse(next.lastAttemptAt!);
      if (lastAt != null) {
        final elapsed = _now().difference(lastAt);
        final required = nextDelay(next.attempts);
        if (elapsed < required) return;
      }
    }

    await _db.markSending(next.id);

    try {
      final repo = await _repo();
      final response = await repo.submit(_toRequest(next));
      // 201 — drop the row and immediately try the next one.
      await _db.deleteRow(next.id);
      // Push the fresh server status into the cached state provider so the
      // home pill / button-set update without a separate refetch.
      _onStatusFresh?.call(response.status);
      // Push the just-synced event into the recently-synced cache so the
      // history view keeps a row visible at the same `occurred_at_client`.
      _onEventSynced?.call(response.event);
      // Recurse to advance the queue.
      _running = false;
      await tick();
      return;
    } on ApiException catch (e) {
      switch (e.code) {
        case 'INVALID_TRANSITION':
        case 'OUT_OF_ORDER':
        case 'TRANSFER_DISABLED':
        case 'NEEDS_PASSWORD_CHANGE':
          await _db.markFailed(
            next.id,
            errorCode: e.code,
            errorMessage: e.message,
          );
          // Skip this row and advance.
          _running = false;
          await tick();
          return;
      }
      if (e.status == 401 || e.code == ApiErrorCode.unauthorized) {
        await _db.markFailed(
          next.id,
          errorCode: e.code,
          errorMessage: e.message,
        );
        // Pause the queue: clearing the auth token kicks the user to /login;
        // the next successful login will resume processing.
        unawaited(_onAuthExpired());
        return;
      }
      // 5xx / network / unknown — return the row to pending, keep attempts,
      // record the error message. The 1s tick will retry once the backoff
      // window has elapsed.
      _log.w('queue tick: retryable failure ${e.code} on row ${next.id}');
      await _db.markPending(
        next.id,
        lastErrorCode: e.code,
        lastErrorMessage: e.message,
      );
      return;
    } catch (e) {
      // Truly unexpected — treat as retryable.
      _log.w('queue tick: unexpected exception on row ${next.id}: $e');
      await _db.markPending(
        next.id,
        lastErrorCode: 'UNKNOWN',
        lastErrorMessage: e.toString(),
      );
      return;
    }
  }

  static SubmitCheckinEventRequest _toRequest(QueueRow row) {
    return SubmitCheckinEventRequest(
      eventType: CheckinEventType.fromJson(row.eventType),
      lat: row.lat,
      lng: row.lng,
      accuracy: row.accuracy,
      manualLabel: row.manualLabel,
      occurredAtClient: row.occurredAtClient,
    );
  }
}

DateTime _defaultNow() => DateTime.now();

final queueProcessorProvider = Provider<QueueProcessor>((ref) {
  final db = ref.watch(checkinQueueDbProvider);
  return QueueProcessor(
    db: db,
    repo: () => ref.read(checkinRepositoryProvider.future),
    isOnline: () => ref.read(connectivityProvider).valueOrNull ?? false,
    onAuthExpired: () async {
      try {
        await ref.read(authProvider.notifier).logout();
      } catch (_) {
        // best-effort
      }
    },
    onStatusFresh: (status) {
      ref.read(checkinStatusProvider.notifier).updateFromServer(status);
    },
    onEventSynced: (event) {
      ref.read(recentlySyncedEventsProvider.notifier).push(event);
    },
  );
});

/// Wires the wake-up triggers: drift change stream, connectivity online,
/// and a 1Hz foreground timer. `keepAlive: true` so the runner stays bound
/// to the lifetime of the Riverpod container, not any one widget.
final queueProcessorRunnerProvider = Provider<void>((ref) {
  ref.keepAlive();
  final processor = ref.watch(queueProcessorProvider);
  final db = ref.watch(checkinQueueDbProvider);

  // Drift change stream: any insert/update/delete on pending_events.
  final dbSub = db.select(db.pendingEvents).watch().listen((_) {
    processor.tick();
  });
  ref.onDispose(dbSub.cancel);

  // Connectivity transition to online.
  ref.listen<AsyncValue<bool>>(connectivityProvider, (prev, next) {
    final wasOnline = prev?.valueOrNull ?? false;
    final isOnline = next.valueOrNull ?? false;
    if (!wasOnline && isOnline) {
      processor.tick();
    }
  });

  // Foreground 1Hz tick.
  final timer = Timer.periodic(const Duration(seconds: 1), (_) {
    processor.tick();
  });
  ref.onDispose(timer.cancel);
});
