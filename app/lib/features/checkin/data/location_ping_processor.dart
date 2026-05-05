import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logger/logger.dart';

import '../../../core/api/api_error.dart';
import '../../../core/api/models/location_ping.dart';
import '../../auth/state/auth_provider.dart';
import '../state/connectivity_provider.dart';
import 'checkin_queue_db.dart';
import 'location_repository.dart';
import 'location_tracking_service.dart';

/// Batch upload for `pending_location_pings`. Different shape from the
/// events `QueueProcessor`: pings have no ordering invariant, so batches
/// of up to 100 are sent in one request and partial-accept response is
/// honored without keeping rejected rows visible (silently deleted).
///
/// Triggers (any of these fires `tick` and the threshold check decides
/// whether to actually flush):
///   • drift change stream on `pending_location_pings`
///   • connectivity transition to online
///   • 1-second foreground timer (cheap, just checks counts / timestamps)
///   • explicit `flushFinal()` from the events `clock_out` finalization
///
/// Threshold conditions (all evaluated at tick time; flush if ANY true):
///   • pendingCount >= 30
///   • now - lastFlushAt >= 5 minutes  (and pendingCount > 0)
///   • _pendingFinal == true
class LocationPingProcessor {
  LocationPingProcessor({
    required CheckinQueueDb db,
    required Future<LocationRepository> Function() repo,
    required bool Function() isOnline,
    Future<void> Function()? onAuthExpired,
    Future<void> Function()? onTrackingDisabled,
    DateTime Function() now = _defaultNow,
  })  : _db = db,
        _repo = repo,
        _isOnline = isOnline,
        _onAuthExpired = onAuthExpired,
        _onTrackingDisabled = onTrackingDisabled,
        _now = now;

  final CheckinQueueDb _db;
  final Future<LocationRepository> Function() _repo;
  final bool Function() _isOnline;
  final Future<void> Function()? _onAuthExpired;
  final Future<void> Function()? _onTrackingDisabled;
  final DateTime Function() _now;
  final Logger _log = Logger();

  bool _running = false;
  bool _pendingFinal = false;
  late DateTime _lastFlushAt = _now();

  static const int _batchSize = 100;
  static const int _countThreshold = 30;
  static const Duration _timeThreshold = Duration(minutes: 5);

  static const List<int> _backoffSeconds = [1, 2, 4, 8, 16, 30];
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

  /// Mark the queue for full drain on the next tick; bypasses the count /
  /// time thresholds. Called by the events processor when it observes a
  /// `clock_out` confirmation.
  void requestFinalFlush() {
    _pendingFinal = true;
  }

  Future<void> _drainOnce() async {
    if (!_isOnline()) return;

    final batch = await _db.pickPendingLocationBatch(_batchSize);
    if (batch.isEmpty) {
      _pendingFinal = false; // nothing to do; clear sticky flag
      return;
    }

    final shouldFlush = _pendingFinal ||
        batch.length >= _countThreshold ||
        _now().difference(_lastFlushAt) >= _timeThreshold;

    if (!shouldFlush) return;

    final ids = batch.map((r) => r.id).toList(growable: false);
    await _db.markLocationSending(ids);

    try {
      final repo = await _repo();
      final response = await repo.submitBatch(
        SubmitLocationPingsRequest(
          pings: batch
              .map((r) => LocationPingInput(
                    lat: r.lat,
                    lng: r.lng,
                    accuracy: r.accuracy,
                    occurredAtClient: r.occurredAtClient,
                  ),)
              .toList(growable: false),
        ),
      );
      // Server returned 201. Delete EVERY in-flight row, regardless of
      // accepted vs rejected — the rejected ones aren't worth surfacing
      // (per design.md "rejected[] rows: mark failed vs delete"). Log a
      // warning for visibility.
      if (response.rejected.isNotEmpty) {
        for (final r in response.rejected) {
          _log.w(
            'Location ping rejected silently — index=${r.index}, '
            'code=${r.code}, message=${r.message}',
          );
        }
      }
      await _db.deleteLocationPings(ids);
      _lastFlushAt = _now();
      // If we hit the batch cap there may be more. Recurse to drain.
      if (batch.length == _batchSize) {
        _running = false;
        await tick();
        return;
      }
      // Drain complete for now. Clear the sticky flag if it was set.
      _pendingFinal = false;
    } on ApiException catch (e) {
      if (e.code == ApiErrorCode.locationTrackingDisabled) {
        // Org flipped the toggle off — these pings will never succeed.
        // Drop everything in flight and stop the tracker.
        await _db.deleteLocationPings(ids);
        _pendingFinal = false;
        unawaited(_onTrackingDisabled?.call() ?? Future.value());
        return;
      }
      if (e.status == 401 || e.code == ApiErrorCode.unauthorized) {
        // Auth expired — drop in-flight, signal auth state, pause.
        await _db.deleteLocationPings(ids);
        _pendingFinal = false;
        unawaited(_onAuthExpired?.call() ?? Future.value());
        return;
      }
      // 5xx / network — return rows to pending, they'll be retried on
      // the next eligible tick. Backoff tracking lives on `attempts` /
      // `last_attempt_at`, like the events queue.
      _log.w('Location batch retryable failure ${e.code}; rows back to pending');
      await _db.markLocationPending(
        ids,
        lastErrorCode: e.code,
        lastErrorMessage: e.message,
      );
    } catch (e) {
      _log.w('Location batch unexpected exception: $e');
      await _db.markLocationPending(
        ids,
        lastErrorCode: 'UNKNOWN',
        lastErrorMessage: e.toString(),
      );
    }
  }
}

DateTime _defaultNow() => DateTime.now();

final locationPingProcessorProvider = Provider<LocationPingProcessor>((ref) {
  final db = ref.watch(checkinQueueDbProvider);
  return LocationPingProcessor(
    db: db,
    repo: () => ref.read(locationRepositoryProvider.future),
    isOnline: () => ref.read(connectivityProvider).valueOrNull ?? false,
    onAuthExpired: () async {
      try {
        await ref.read(authProvider.notifier).logout();
      } catch (_) {
        // best-effort
      }
    },
    onTrackingDisabled: () async {
      // Have the controller stop the tracker — it'll re-evaluate on the
      // next status emission, but this is faster.
      final service = ref.read(locationTrackingServiceProvider);
      try {
        await service.stop();
      } catch (_) {
        // best-effort
      }
    },
  );
});

/// Wires drift change stream + connectivity transitions + 1Hz timer to
/// `tick()`. Kept-alive so the processor stays bound to the container.
final locationPingProcessorRunnerProvider = Provider<void>((ref) {
  ref.keepAlive();
  final processor = ref.watch(locationPingProcessorProvider);
  final db = ref.watch(checkinQueueDbProvider);

  final dbSub = db.watchAllLocationPings().listen((_) {
    processor.tick();
  });
  ref.onDispose(dbSub.cancel);

  ref.listen<AsyncValue<bool>>(connectivityProvider, (prev, next) {
    final wasOnline = prev?.valueOrNull ?? false;
    final isOnline = next.valueOrNull ?? false;
    if (!wasOnline && isOnline) {
      processor.tick();
    }
  });

  final timer = Timer.periodic(const Duration(seconds: 1), (_) {
    processor.tick();
  });
  ref.onDispose(timer.cancel);
});
