import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';

import '../data/geolocation_service.dart';

/// Holds the current `LocationPermission` state. Callers re-check on
/// app resume (the user might have toggled the OS setting) and after
/// in-app `requestPermission()`.
class LocationPermissionNotifier extends AsyncNotifier<LocationPermission> {
  @override
  Future<LocationPermission> build() async {
    final svc = ref.read(geolocationServiceProvider);
    return svc.currentPermission();
  }

  Future<LocationPermission> refresh() async {
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(() async {
      final svc = ref.read(geolocationServiceProvider);
      return svc.currentPermission();
    });
    return state.value ?? LocationPermission.denied;
  }

  Future<LocationPermission> request() async {
    final svc = ref.read(geolocationServiceProvider);
    final result = await svc.requestPermission();
    state = AsyncValue<LocationPermission>.data(result);
    return result;
  }
}

final locationPermissionProvider =
    AsyncNotifierProvider<LocationPermissionNotifier, LocationPermission>(
  LocationPermissionNotifier.new,
);
