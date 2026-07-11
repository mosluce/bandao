import 'dart:async';
import 'dart:io' show Platform;

import 'package:drift/drift.dart' show Value;
import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:logger/logger.dart';

import '../../../core/api/models/submit_checkin_event.dart';
import '../../../core/storage/secure_storage.dart';
import 'checkin_queue_db.dart';

/// Wraps `Geolocator.getPositionStream` to provide a single-source-of-truth
/// "is this device currently emitting position pings into drift?" service.
///
/// Lifecycle is managed by `LocationTrackingController` — this class is
/// pure I/O, no decision-making. Tests substitute a fake to assert that the
/// controller calls `start` / `stop` at the right moments.
abstract class LocationTrackingService {
  bool get isActive;
  DateTime? get startedAt;
  Stream<DateTime> get tickStream;
  Future<void> start({required String appUserId});
  Future<void> stop();
}

class GeolocatorTrackingService implements LocationTrackingService {
  GeolocatorTrackingService(this._db, this._secureStorage);

  final CheckinQueueDb _db;
  final SecureStorage _secureStorage;
  final Logger _log = Logger();

  StreamSubscription<Position>? _sub;
  Timer? _tickTimer;
  StreamController<DateTime>? _tickStream;
  DateTime? _startedAt;
  DateTime? _lastEnqueuedAt;
  String? _appUserId;

  /// 60s minimum interval between enqueues — the AND condition with the
  /// OS-level 100m distance filter.
  static const Duration _throttle = Duration(seconds: 60);

  @override
  bool get isActive => _sub != null;

  @override
  DateTime? get startedAt => _startedAt;

  @override
  Stream<DateTime> get tickStream =>
      (_tickStream ??= StreamController<DateTime>.broadcast()).stream;

  @override
  Future<void> start({required String appUserId}) async {
    if (isActive) return;
    _appUserId = appUserId;
    _startedAt = DateTime.now();
    _lastEnqueuedAt = null;

    // Clear the clean-stop flag — set when stop() runs cleanly. A force-quit
    // would leave whatever the previous stop wrote (or nothing), and the
    // recovery banner check on next boot is what surfaces it.
    await _secureStorage.clearLocationTrackingLastCleanStop();

    final settings = _platformSettings();
    _sub = Geolocator.getPositionStream(locationSettings: settings)
        .listen(_onPosition, onError: _onError);

    // Per-second tick stream so the chip can rebuild its elapsed counter.
    _tickStream ??= StreamController<DateTime>.broadcast();
    _tickTimer = Timer.periodic(const Duration(seconds: 1), (_) {
      _tickStream?.add(DateTime.now());
    });
  }

  @override
  Future<void> stop() async {
    if (!isActive) return;
    await _sub?.cancel();
    _sub = null;
    _tickTimer?.cancel();
    _tickTimer = null;
    _startedAt = null;
    _lastEnqueuedAt = null;
    _appUserId = null;
    await _secureStorage.writeLocationTrackingLastCleanStop(DateTime.now());
  }

  @visibleForTesting
  Future<void> handlePositionForTest(Position pos, String appUserId) {
    _appUserId = appUserId;
    return _onPosition(pos);
  }

  Future<void> _onPosition(Position pos) async {
    final appUserId = _appUserId;
    if (appUserId == null) return; // stop racing — drop

    final now = DateTime.now();
    if (_lastEnqueuedAt != null &&
        now.difference(_lastEnqueuedAt!) < _throttle) {
      return; // throttle
    }
    _lastEnqueuedAt = now;

    try {
      await _db.enqueueLocationPing(
        PendingLocationPingsCompanion(
          appUserId: Value(appUserId),
          lat: Value(pos.latitude),
          lng: Value(pos.longitude),
          accuracy: Value(pos.accuracy.isFinite ? pos.accuracy : null),
          occurredAtClient: Value(nowOccurredAtClient(now)),
          enqueuedAt: Value(now.toIso8601String()),
        ),
      );
    } catch (e) {
      _log.w('Failed to enqueue location ping: $e');
    }
  }

  void _onError(Object err) {
    // OS revoked permission, GPS hardware error, etc. v1 just logs — v2
    // can surface a chip state change. The stream may also emit further
    // values after this; we don't tear down here.
    _log.w('Location stream error: $err');
  }

  LocationSettings _platformSettings() {
    if (Platform.isIOS) {
      return AppleSettings(
        accuracy: LocationAccuracy.high,
        distanceFilter: 100,
        pauseLocationUpdatesAutomatically: false,
        showBackgroundLocationIndicator: true,
        activityType: ActivityType.other,
      );
    }
    if (Platform.isAndroid) {
      return AndroidSettings(
        accuracy: LocationAccuracy.high,
        distanceFilter: 100,
        foregroundNotificationConfig: const ForegroundNotificationConfig(
          notificationTitle: '班到',
          notificationText: '工作期間定位追蹤中',
          enableWakeLock: true,
          setOngoing: true,
        ),
      );
    }
    return const LocationSettings(
      accuracy: LocationAccuracy.high,
      distanceFilter: 100,
    );
  }
}

final locationTrackingServiceProvider =
    Provider<LocationTrackingService>((ref) {
  final db = ref.watch(checkinQueueDbProvider);
  final storage = ref.watch(secureStorageProvider);
  return GeolocatorTrackingService(db, storage);
});
