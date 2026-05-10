import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

/// Storage keys used by the app. Locked to three for v1 — bearer token,
/// last successful org_code (prefill on next login), and the debug-only
/// API base URL override.
class SecureStorageKeys {
  const SecureStorageKeys._();

  static const String bearerToken = 'auth.bearer_token';
  static const String lastOrgCode = 'auth.last_org_code';
  static const String apiBaseUrlOverride = 'dev.api_base_url_override';
  static const String backgroundSyncTipSeen = 'home.background_sync_tip_seen';
  static const String locationTrackingLastCleanStop =
      'bandao.location_tracking.last_clean_stop';
  static const String privacyUrlOverride = 'dev.privacy_url_override';

  /// Per-AppUser consent flag — formatted as
  /// `bandao.location_tracking.consent.<app_user_id>`.
  static String locationTrackingConsentKey(String appUserId) =>
      'bandao.location_tracking.consent.$appUserId';
}

/// Thin typed wrapper around `flutter_secure_storage`. The wrapper exists so
/// the rest of the app does not depend on the keys directly and so that tests
/// can plug in an in-memory fake without faking the whole flutter plugin.
///
/// **Bearer token invariants (DO NOT bypass this wrapper):**
///
/// - `auth.bearer_token` is cached in process memory after the first
///   successful read. All reads/writes/clears MUST go through this wrapper —
///   any direct `FlutterSecureStorage` access for that key would let the
///   in-memory cache drift from persistent state.
/// - The default underlying storage is constructed with
///   `IOSOptions(accessibility: KeychainAccessibility.first_unlock)` so the
///   token survives device-lock once the user has unlocked the device at
///   least once after reboot. This is what keeps the user logged in when
///   iOS keeps the app alive in the background (location tracking) while
///   the screen is locked.
class SecureStorage {
  SecureStorage([FlutterSecureStorage? storage])
      : _storage = storage ?? const FlutterSecureStorage(iOptions: _iosOptions);

  static const IOSOptions _iosOptions = IOSOptions(
    accessibility: KeychainAccessibility.first_unlock,
  );

  /// Test hook: lets unit tests assert the iOS Keychain accessibility class
  /// the wrapper applies to its default storage.
  @visibleForTesting
  static IOSOptions get defaultIosOptionsForTest => _iosOptions;

  final FlutterSecureStorage _storage;

  String? _cachedToken;
  bool _tokenLoaded = false;

  Future<String?> readToken() async {
    if (_tokenLoaded) return _cachedToken;
    final value = await _storage.read(key: SecureStorageKeys.bearerToken);
    _cachedToken = value;
    _tokenLoaded = true;
    return value;
  }

  Future<void> writeToken(String token) async {
    await _storage.write(key: SecureStorageKeys.bearerToken, value: token);
    _cachedToken = token;
    _tokenLoaded = true;
  }

  Future<void> clearToken() async {
    await _storage.delete(key: SecureStorageKeys.bearerToken);
    _cachedToken = null;
    _tokenLoaded = true;
  }

  Future<String?> readLastOrgCode() =>
      _storage.read(key: SecureStorageKeys.lastOrgCode);

  Future<void> writeLastOrgCode(String orgCode) => _storage.write(
        key: SecureStorageKeys.lastOrgCode,
        value: orgCode,
      );

  Future<void> clearLastOrgCode() =>
      _storage.delete(key: SecureStorageKeys.lastOrgCode);

  Future<String?> readApiBaseUrlOverride() =>
      _storage.read(key: SecureStorageKeys.apiBaseUrlOverride);

  Future<void> writeApiBaseUrlOverride(String url) => _storage.write(
        key: SecureStorageKeys.apiBaseUrlOverride,
        value: url,
      );

  Future<void> clearApiBaseUrlOverride() =>
      _storage.delete(key: SecureStorageKeys.apiBaseUrlOverride);

  Future<bool> readBackgroundSyncTipSeen() async {
    final v = await _storage.read(key: SecureStorageKeys.backgroundSyncTipSeen);
    return v == 'true';
  }

  Future<void> markBackgroundSyncTipSeen() => _storage.write(
        key: SecureStorageKeys.backgroundSyncTipSeen,
        value: 'true',
      );

  Future<DateTime?> readLocationTrackingLastCleanStop() async {
    final v = await _storage.read(
      key: SecureStorageKeys.locationTrackingLastCleanStop,
    );
    if (v == null || v.isEmpty) return null;
    return DateTime.tryParse(v);
  }

  Future<void> writeLocationTrackingLastCleanStop(DateTime t) => _storage.write(
        key: SecureStorageKeys.locationTrackingLastCleanStop,
        value: t.toIso8601String(),
      );

  Future<void> clearLocationTrackingLastCleanStop() => _storage.delete(
        key: SecureStorageKeys.locationTrackingLastCleanStop,
      );

  Future<bool> readLocationTrackingConsent(String appUserId) async {
    final v = await _storage.read(
      key: SecureStorageKeys.locationTrackingConsentKey(appUserId),
    );
    return v == 'true';
  }

  Future<void> writeLocationTrackingConsent(String appUserId) => _storage.write(
        key: SecureStorageKeys.locationTrackingConsentKey(appUserId),
        value: 'true',
      );

  Future<String?> readPrivacyUrlOverride() =>
      _storage.read(key: SecureStorageKeys.privacyUrlOverride);

  Future<void> writePrivacyUrlOverride(String url) => _storage.write(
        key: SecureStorageKeys.privacyUrlOverride,
        value: url,
      );

  Future<void> clearPrivacyUrlOverride() => _storage.delete(
        key: SecureStorageKeys.privacyUrlOverride,
      );
}

/// Riverpod provider so consumers can `ref.read(secureStorageProvider)`.
final secureStorageProvider = Provider<SecureStorage>((ref) {
  return SecureStorage();
});
