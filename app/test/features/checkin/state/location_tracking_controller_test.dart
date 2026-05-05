import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/checkin_status.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/features/checkin/data/location_tracking_service.dart';
import 'package:argus_app/features/checkin/state/checkin_status_provider.dart';
import 'package:argus_app/features/checkin/state/effective_status_provider.dart';
import 'package:argus_app/features/checkin/state/location_tracking_controller.dart';

import '../../../helpers/fake_auth_notifier.dart';

void main() {
  group('LocationTrackingController', () {
    test('off_duty + empty queue → tracker stays stopped', () async {
      final svc = _FakeService();
      final container = _container(
        svc: svc,
        serverStatus: AppUserCheckinStatus.offDuty,
        effective: AppUserCheckinStatus.offDuty,
      );

      // Bootstrap controller.
      container.read(locationTrackingControllerProvider);
      await pumpEventQueue();

      expect(svc.startCalls, 0);
      expect(svc.stopCalls, 0);
    });

    test('cold-start with on_site → tracker starts', () async {
      final svc = _FakeService();
      final statusNotifier = _FakeStatusNotifier(AppUserCheckinStatus.onSite);
      final effective = ValueNotifier<AppUserCheckinStatus>(
        AppUserCheckinStatus.onSite,
      );
      final container = _containerCustom(
        svc: svc,
        status: statusNotifier,
        effective: effective,
      );

      container.read(locationTrackingControllerProvider);
      // Wait for the AsyncNotifier to resolve so the listener fires.
      await container.read(checkinStatusProvider.future);
      // Re-emit current state to trigger listener (fireImmediately catches
      // the loading state, post-build resolution doesn't always re-fire on
      // overridden notifiers).
      statusNotifier.setStatus(AppUserCheckinStatus.onSite);
      await pumpEventQueue();

      expect(svc.startCalls, 1);
      expect(svc.stopCalls, 0);
    });

    test('pending clock_in (effective on_site, server off_duty) does NOT start',
        () async {
      final svc = _FakeService();
      final container = _container(
        svc: svc,
        serverStatus: AppUserCheckinStatus.offDuty,
        effective: AppUserCheckinStatus.onSite, // optimistic from queue
      );

      container.read(locationTrackingControllerProvider);
      await pumpEventQueue();

      expect(svc.startCalls, 0);
    });

    test('server-confirmed transition off_duty → on_site starts tracker',
        () async {
      final svc = _FakeService();
      final statusNotifier = _FakeStatusNotifier(AppUserCheckinStatus.offDuty);
      final effective = ValueNotifier<AppUserCheckinStatus>(
        AppUserCheckinStatus.offDuty,
      );

      final container = _containerCustom(svc: svc, status: statusNotifier, effective: effective);
      container.read(locationTrackingControllerProvider);
      // Pre-warm auth so the on_site fire later finds AuthAuthenticated.
      await container.read(authProvider.future);
      await container.read(checkinStatusProvider.future);
      // Trigger initial off_duty resolution so controller is fully wired.
      statusNotifier.setStatus(AppUserCheckinStatus.offDuty);
      await pumpEventQueue();
      expect(svc.startCalls, 0, reason: 'off_duty does not start');

      // server confirms clock_in
      statusNotifier.setStatus(AppUserCheckinStatus.onSite);
      effective.value = AppUserCheckinStatus.onSite;
      await pumpEventQueue();

      expect(svc.startCalls, 1);
    });

    test('effective off_duty (clock_out tap) stops tracker', () async {
      final svc = _FakeService();
      final statusNotifier = _FakeStatusNotifier(AppUserCheckinStatus.onSite);
      final effective = ValueNotifier<AppUserCheckinStatus>(
        AppUserCheckinStatus.onSite,
      );

      final container = _containerCustom(svc: svc, status: statusNotifier, effective: effective);
      container.read(locationTrackingControllerProvider);
      await container.read(checkinStatusProvider.future);
      // Re-emit to force the listener to wake from initial loading state.
      statusNotifier.setStatus(AppUserCheckinStatus.onSite);
      await pumpEventQueue();

      expect(svc.startCalls, 1);

      // Worker taps [下班], optimistic effective flips before server.
      effective.value = AppUserCheckinStatus.offDuty;
      await pumpEventQueue();

      expect(svc.stopCalls, 1);
    });

    test('transfer events do not cycle the tracker', () async {
      final svc = _FakeService();
      final statusNotifier = _FakeStatusNotifier(AppUserCheckinStatus.onSite);
      final effective = ValueNotifier<AppUserCheckinStatus>(
        AppUserCheckinStatus.onSite,
      );

      final container = _containerCustom(svc: svc, status: statusNotifier, effective: effective);
      container.read(locationTrackingControllerProvider);
      await container.read(checkinStatusProvider.future);
      await pumpEventQueue();

      // server status: on_site → in_transit → on_site
      statusNotifier.setStatus(AppUserCheckinStatus.inTransit);
      effective.value = AppUserCheckinStatus.inTransit;
      await pumpEventQueue();

      statusNotifier.setStatus(AppUserCheckinStatus.onSite);
      effective.value = AppUserCheckinStatus.onSite;
      await pumpEventQueue();

      expect(svc.startCalls, 1, reason: 'no extra start');
      expect(svc.stopCalls, 0, reason: 'no stop during transfer');
    });

    test('Org toggle off + on_site does NOT start tracker', () async {
      final svc = _FakeService();
      final container = _container(
        svc: svc,
        serverStatus: AppUserCheckinStatus.onSite,
        effective: AppUserCheckinStatus.onSite,
        locationTrackingEnabled: false,
      );

      container.read(locationTrackingControllerProvider);
      await container.read(authProvider.future);
      await container.read(checkinStatusProvider.future);
      await pumpEventQueue();

      expect(svc.startCalls, 0, reason: 'toggle off blocks start');
    });
  });
}

Future<void> pumpEventQueue() async {
  // Multiple microtask flushes — AsyncNotifier resolution + ref.listen
  // dispatch + service.start `then` callback can stack up to 3 deep.
  for (var i = 0; i < 5; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

ProviderContainer _container({
  required _FakeService svc,
  required AppUserCheckinStatus serverStatus,
  required AppUserCheckinStatus effective,
  bool locationTrackingEnabled = true,
}) {
  return _containerCustom(
    svc: svc,
    status: _FakeStatusNotifier(serverStatus),
    effective: ValueNotifier<AppUserCheckinStatus>(effective),
    locationTrackingEnabled: locationTrackingEnabled,
  );
}

ProviderContainer _containerCustom({
  required _FakeService svc,
  required _FakeStatusNotifier status,
  required ValueNotifier<AppUserCheckinStatus> effective,
  bool locationTrackingEnabled = true,
}) {
  final auth = AuthState.authenticated(
    user: const AppUser(
      id: 'u1',
      username: 'alice',
      displayName: 'Alice',
      status: AppUserStatus.active,
      needsPasswordChange: false,
      createdAt: '2025-01-01T00:00:00Z',
    ),
    org: Org(
      id: 'o1',
      name: 'Acme',
      code: 'ABCDEFGHIJ',
      ownerId: 'u1',
      timezone: 'Asia/Taipei',
      checkin: OrgCheckin(
        transferEnabled: true,
        locationTrackingEnabled: locationTrackingEnabled,
      ),
    ),
    needsPasswordChange: false,
  );
  final container = ProviderContainer(
    overrides: [
      authProvider.overrideWith(
        () => FakeAuthNotifier(AsyncValue.data(auth)),
      ),
      locationTrackingServiceProvider.overrideWithValue(svc),
      checkinStatusProvider.overrideWith(() => status),
      effectiveStatusProvider.overrideWith(
        (ref) => EffectiveStatus(
          status: effective.value,
          hasPendingTransition: false,
        ),
      ),
    ],
  );
  // Re-emit when the ValueNotifier changes.
  effective.addListener(() {
    container.invalidate(effectiveStatusProvider);
  });
  addTearDown(container.dispose);
  return container;
}

class _FakeService implements LocationTrackingService {
  bool _isActive = false;
  int startCalls = 0;
  int stopCalls = 0;

  @override
  bool get isActive => _isActive;
  @override
  DateTime? get startedAt => _isActive ? DateTime.now() : null;
  @override
  Stream<DateTime> get tickStream => const Stream<DateTime>.empty();

  @override
  Future<void> start({required String appUserId}) async {
    if (_isActive) return;
    _isActive = true;
    startCalls++;
  }

  @override
  Future<void> stop() async {
    if (!_isActive) return;
    _isActive = false;
    stopCalls++;
  }
}

class _FakeStatusNotifier extends CheckinStatusNotifier {
  _FakeStatusNotifier(this._status);

  AppUserCheckinStatus _status;

  void setStatus(AppUserCheckinStatus s) {
    _status = s;
    state = AsyncValue.data(_dto());
  }

  CheckinUserStatusDto _dto() => CheckinUserStatusDto(
        appUserId: 'u1',
        status: _status,
        hasSkewWarning: false,
      );

  @override
  Future<CheckinUserStatusDto?> build() async => _dto();
}

// Avoid pulling in flutter material for ValueNotifier.
class ValueNotifier<T> {
  ValueNotifier(this._value);
  T _value;
  final List<void Function()> _listeners = <void Function()>[];

  T get value => _value;
  set value(T v) {
    _value = v;
    for (final l in List.of(_listeners)) {
      l();
    }
  }

  void addListener(void Function() l) => _listeners.add(l);
}
