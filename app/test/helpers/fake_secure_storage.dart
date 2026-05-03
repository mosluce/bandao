import 'package:argus_app/core/storage/secure_storage.dart';

/// In-memory SecureStorage stand-in for widget tests. Keeps the surface
/// `implements`-only so we don't drag in flutter_secure_storage's plugin
/// channel during tests.
class FakeSecureStorage implements SecureStorage {
  FakeSecureStorage({
    String? token,
    String? orgCode,
    String? apiBaseUrlOverride,
  })  : _token = token,
        _orgCode = orgCode,
        _override = apiBaseUrlOverride;

  String? _token;
  String? _orgCode;
  String? _override;

  @override
  Future<String?> readToken() async => _token;

  @override
  Future<void> writeToken(String value) async => _token = value;

  @override
  Future<void> clearToken() async => _token = null;

  @override
  Future<String?> readLastOrgCode() async => _orgCode;

  @override
  Future<void> writeLastOrgCode(String value) async => _orgCode = value;

  @override
  Future<void> clearLastOrgCode() async => _orgCode = null;

  @override
  Future<String?> readApiBaseUrlOverride() async => _override;

  @override
  Future<void> writeApiBaseUrlOverride(String url) async => _override = url;

  @override
  Future<void> clearApiBaseUrlOverride() async => _override = null;
}
