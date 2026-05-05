import 'package:app_settings/app_settings.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';

import '../../../core/api/models/checkin_event.dart';

/// Typed wrapper around `geolocator` + `app_settings`. Tests substitute a
/// fake via the Riverpod override.
abstract class GeolocationService {
  Future<LocationPermission> currentPermission();
  Future<LocationPermission> requestPermission();

  /// Capture the device's current coordinates with `LocationAccuracy.high` and
  /// a 10-second timeout, falling back to `getLastKnownPosition()` on timeout
  /// or failure. Returns null when neither path produces coordinates — the
  /// caller MUST refuse to enqueue in that case (see spec 1.2 / 1.3 scenarios).
  Future<({GeoPoint point, double? accuracyMeters})?> capture();

  Future<bool> openSettings();
}

class GeolocatorService implements GeolocationService {
  const GeolocatorService();

  @override
  Future<LocationPermission> currentPermission() =>
      Geolocator.checkPermission();

  @override
  Future<LocationPermission> requestPermission() =>
      Geolocator.requestPermission();

  @override
  Future<({GeoPoint point, double? accuracyMeters})?> capture() async {
    try {
      final pos = await Geolocator.getCurrentPosition(
        locationSettings: const LocationSettings(
          accuracy: LocationAccuracy.high,
          timeLimit: Duration(seconds: 10),
        ),
      );
      return (
        point: GeoPoint(lat: pos.latitude, lng: pos.longitude),
        accuracyMeters: pos.accuracy,
      );
    } catch (_) {
      // fallthrough to last-known
    }
    try {
      final last = await Geolocator.getLastKnownPosition();
      if (last == null) return null;
      return (
        point: GeoPoint(lat: last.latitude, lng: last.longitude),
        accuracyMeters: last.accuracy,
      );
    } catch (_) {
      return null;
    }
  }

  @override
  Future<bool> openSettings() async {
    await AppSettings.openAppSettings();
    return true;
  }
}

final geolocationServiceProvider = Provider<GeolocationService>((_) {
  return const GeolocatorService();
});
