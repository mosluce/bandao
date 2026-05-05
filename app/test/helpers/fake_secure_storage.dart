import 'package:argus_app/core/storage/secure_storage.dart';

/// In-memory SecureStorage stand-in for widget tests. Keeps the surface
/// `implements`-only so we don't drag in flutter_secure_storage's plugin
/// channel during tests.
class FakeSecureStorage implements SecureStorage {
  FakeSecureStorage({
    String? token,
    String? orgCode,
    String? apiBaseUrlOverride,
    bool backgroundSyncTipSeen = false,
  })  : _token = token,
        _orgCode = orgCode,
        _override = apiBaseUrlOverride,
        _bgTipSeen = backgroundSyncTipSeen;

  String? _token;
  String? _orgCode;
  String? _override;
  bool _bgTipSeen;

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

  @override
  Future<bool> readBackgroundSyncTipSeen() async => _bgTipSeen;

  @override
  Future<void> markBackgroundSyncTipSeen() async => _bgTipSeen = true;

  DateTime? _trackingLastCleanStop;
  final Map<String, bool> _trackingConsent = <String, bool>{};
  String? _privacyUrlOverride;

  @override
  Future<DateTime?> readLocationTrackingLastCleanStop() async =>
      _trackingLastCleanStop;

  @override
  Future<void> writeLocationTrackingLastCleanStop(DateTime t) async =>
      _trackingLastCleanStop = t;

  @override
  Future<void> clearLocationTrackingLastCleanStop() async =>
      _trackingLastCleanStop = null;

  @override
  Future<bool> readLocationTrackingConsent(String appUserId) async =>
      _trackingConsent[appUserId] ?? false;

  @override
  Future<void> writeLocationTrackingConsent(String appUserId) async =>
      _trackingConsent[appUserId] = true;

  @override
  Future<String?> readPrivacyUrlOverride() async => _privacyUrlOverride;

  @override
  Future<void> writePrivacyUrlOverride(String url) async =>
      _privacyUrlOverride = url;

  @override
  Future<void> clearPrivacyUrlOverride() async => _privacyUrlOverride = null;
}
