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
}

/// Thin typed wrapper around `flutter_secure_storage`. The wrapper exists so
/// the rest of the app does not depend on the keys directly and so that tests
/// can plug in an in-memory fake without faking the whole flutter plugin.
class SecureStorage {
  SecureStorage([FlutterSecureStorage? storage])
      : _storage = storage ?? const FlutterSecureStorage();

  final FlutterSecureStorage _storage;

  Future<String?> readToken() =>
      _storage.read(key: SecureStorageKeys.bearerToken);

  Future<void> writeToken(String token) =>
      _storage.write(key: SecureStorageKeys.bearerToken, value: token);

  Future<void> clearToken() =>
      _storage.delete(key: SecureStorageKeys.bearerToken);

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
}

/// Riverpod provider so consumers can `ref.read(secureStorageProvider)`.
final secureStorageProvider = Provider<SecureStorage>((ref) {
  return SecureStorage();
});
