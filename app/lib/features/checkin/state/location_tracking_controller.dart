import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logger/logger.dart';

import '../../../core/api/models/checkin_status.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../data/location_tracking_service.dart';
import 'checkin_status_provider.dart';
import 'effective_status_provider.dart';

/// Owns the start / stop decision for the `LocationTrackingService`.
///
/// Asymmetric trigger model — see `add-location-tracking-app/design.md`:
///   • **Start** when the SERVER-CONFIRMED status (from
///     `checkinStatusProvider`) is anything but `off_duty`. Conservative —
///     a pending-but-unconfirmed clock_in does NOT start the tracker.
///     Cold start with an existing on_site / in_transit shift naturally
///     fires through this same path.
///   • **Stop** when the EFFECTIVE status (from
///     `effectiveStatusProvider`) is `off_duty`. Optimistic — a tap on
///     `[下班]` flips the effective state immediately, so the tracker
///     shuts down before the server confirms. If clock_out fails and the
///     effective state rolls back, the start path picks up again.
///
/// Both `_maybeStart` and `_maybeStop` are idempotent — it's safe to
/// trigger them on every provider emission without de-duplication
/// at the call site.
class LocationTrackingController extends Notifier<bool> {
  final Logger _log = Logger();

  @override
  bool build() {
    final service = ref.watch(locationTrackingServiceProvider);

    ref.listen<AsyncValue<CheckinUserStatusDto?>>(
      checkinStatusProvider,
      (prev, next) {
        final value = next.valueOrNull;
        if (value == null) return;
        if (value.status != AppUserCheckinStatus.offDuty) {
          _maybeStart(service);
        }
      },
      fireImmediately: true,
    );

    ref.listen<EffectiveStatus>(
      effectiveStatusProvider,
      (prev, next) {
        if (next.status == AppUserCheckinStatus.offDuty) {
          _maybeStop(service);
        }
      },
      fireImmediately: true,
    );

    // Watch the Org toggle — if admin flips it off mid-shift, stop the
    // tracker locally rather than waiting up to 5 minutes for the next
    // batch flush to be 403-rejected by the server.
    ref.listen<AsyncValue<AuthState>>(
      authProvider,
      (prev, next) {
        final auth = next.valueOrNull;
        if (auth is! AuthAuthenticated) return;
        if (!auth.org.checkin.locationTrackingEnabled) {
          _maybeStop(service);
        }
      },
      fireImmediately: true,
    );

    // Initial state from the service in case it survived a hot reload.
    return service.isActive;
  }

  DateTime? get startedAt =>
      ref.read(locationTrackingServiceProvider).startedAt;

  Stream<DateTime> get tickStream =>
      ref.read(locationTrackingServiceProvider).tickStream;

  void _maybeStart(LocationTrackingService service) {
    if (service.isActive) return;
    final auth = ref.read(authProvider).valueOrNull;
    if (auth is! AuthAuthenticated) return;
    if (!auth.org.checkin.locationTrackingEnabled) return;
    final appUserId = auth.user.id;
    _log.i('LocationTrackingController: starting for $appUserId');
    // Fire-and-forget; the service guards re-entrancy.
    service.start(appUserId: appUserId).then((_) {
      state = true;
    }).catchError((Object e) {
      _log.w('LocationTrackingController.start failed: $e');
    });
  }

  void _maybeStop(LocationTrackingService service) {
    if (!service.isActive) return;
    _log.i('LocationTrackingController: stopping');
    service.stop().then((_) {
      state = false;
    }).catchError((Object e) {
      _log.w('LocationTrackingController.stop failed: $e');
    });
  }
}

final locationTrackingControllerProvider =
    NotifierProvider<LocationTrackingController, bool>(
  LocationTrackingController.new,
);
