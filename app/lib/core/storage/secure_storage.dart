import 'package:flutter/foundation.dart' show visibleForTesting;
import 'package:flutter/services.dart' show PlatformException;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import '../telemetry/error_reporter.dart';

/// Storage keys used by the app. Locked to three for v1 — bearer token,
/// last successful org_code (prefill on next login), and the per-device
/// API base URL override (self-hosted server support, all build modes).
class SecureStorageKeys {
  const SecureStorageKeys._();

  static const String bearerToken = 'auth.bearer_token';
  static const String lastOrgCode = 'auth.last_org_code';
  static const String apiBaseUrlOverride = 'server.api_base_url';
  static const String backgroundSyncTipSeen = 'home.background_sync_tip_seen';
  static const String locationTrackingLastCleanStop =
      'bandao.location_tracking.last_clean_stop';
  static const String privacyUrlOverride = 'dev.privacy_url_override';

  /// The AppUser's recent check-in labels, JSON-encoded. Cached so the
  /// suggestions survive going offline — a worker out of signal still has to
  /// label their check-in, and the list is otherwise derived from a fetch.
  static const String recentCheckinLabels = 'checkin.recent_labels';

  /// Per-AppUser consent flag — formatted as
  /// `bandao.location_tracking.consent.<app_user_id>`.
  static String locationTrackingConsentKey(String appUserId) =>
      'bandao.location_tracking.consent.$appUserId';
}

/// Which platform operation failed. Reads are recoverable in place (the value
/// is treated as absent); writes and deletes are surfaced to the caller.
enum SecureStorageOperation { read, write, delete }

/// A platform keystore operation failed.
///
/// Deliberately a typed exception rather than a silently-absorbed failure or a
/// `bool` return. Absorbing it is what made this class of bug invisible on the
/// read path for so long — a keystore that cannot be read looks exactly like
/// one holding nothing — and a `bool` is trivially ignored by the next caller
/// added. See openspec `app-shell`, "Secure-storage writes and deletes fail
/// loudly and non-fatally".
class SecureStorageFailure implements Exception {
  const SecureStorageFailure({
    required this.key,
    required this.operation,
    required this.cause,
  });

  /// The storage key involved, so a report identifies which entry is broken.
  final String key;
  final SecureStorageOperation operation;

  /// The underlying platform error (typically a `PlatformException`).
  final Object cause;

  @override
  String toString() =>
      'SecureStorageFailure(${operation.name} `$key`): $cause';
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
  /// [reporter] defaults to a no-op so objects built outside the provider
  /// graph (unit tests) never need Firebase. Production construction goes
  /// through [secureStorageProvider], which injects the real reporter.
  SecureStorage([
    FlutterSecureStorage? storage,
    ErrorReporter reporter = const NoopErrorReporter(),
  ])  : _storage = storage ?? const FlutterSecureStorage(iOptions: _iosOptions),
        _reporter = reporter;

  static const IOSOptions _iosOptions = IOSOptions(
    accessibility: KeychainAccessibility.first_unlock,
  );

  /// Test hook: lets unit tests assert the iOS Keychain accessibility class
  /// the wrapper applies to its default storage.
  @visibleForTesting
  static IOSOptions get defaultIosOptionsForTest => _iosOptions;

  final FlutterSecureStorage _storage;
  final ErrorReporter _reporter;

  String? _cachedToken;
  bool _tokenLoaded = false;

  void _report(SecureStorageFailure failure, StackTrace stackTrace) {
    _reporter.recordNonFatal(
      failure,
      stackTrace,
      reason: 'secure storage ${failure.operation.name} failed',
      context: <String, Object?>{
        'secure_storage_key': failure.key,
        'secure_storage_operation': failure.operation.name,
      },
    );
  }

  /// Reads [key], treating an undecryptable entry as absent rather than
  /// crashing. On Android the plugin's AES key lives in the Keystore; an OS
  /// upgrade, a restore to a different device, or a lock-screen credential
  /// reset can invalidate it while the (now-undecryptable) ciphertext stays
  /// in SharedPreferences. Every future read of that key then throws a
  /// `PlatformException` wrapping `BadPaddingException`. We drop the
  /// corrupted entry so the app can still boot instead of crashing on
  /// startup screens like `/server-config`.
  Future<String?> _safeRead(String key) async {
    try {
      return await _storage.read(key: key);
    } on PlatformException catch (e, st) {
      // Stays fail-soft — the app must still boot — but no longer silent.
      // Discarding this is why a keystore that cannot be read was
      // indistinguishable from one holding nothing, which hid the real
      // problem behind an apparent login bug.
      _report(
        SecureStorageFailure(
          key: key,
          operation: SecureStorageOperation.read,
          cause: e,
        ),
        st,
      );
      try {
        await _storage.delete(key: key);
      } on PlatformException {
        // The corrupted entry stays. Nothing further to do; the read still
        // resolves as absent, which is what the caller needs.
      }
      return null;
    }
  }

  /// Writes [key], converting a platform failure into [SecureStorageFailure]
  /// so no raw plugin exception escapes this wrapper.
  Future<void> _safeWrite(String key, String value) async {
    try {
      await _storage.write(key: key, value: value);
    } on PlatformException catch (e, st) {
      final failure = SecureStorageFailure(
        key: key,
        operation: SecureStorageOperation.write,
        cause: e,
      );
      _report(failure, st);
      throw failure;
    }
  }

  /// Deletes [key], converting a platform failure into [SecureStorageFailure].
  /// Callers that must proceed regardless (logout) catch it explicitly.
  Future<void> _safeDelete(String key) async {
    try {
      await _storage.delete(key: key);
    } on PlatformException catch (e, st) {
      final failure = SecureStorageFailure(
        key: key,
        operation: SecureStorageOperation.delete,
        cause: e,
      );
      _report(failure, st);
      throw failure;
    }
  }

  Future<String?> readToken() async {
    if (_tokenLoaded) return _cachedToken;
    final value = await _safeRead(SecureStorageKeys.bearerToken);
    _cachedToken = value;
    _tokenLoaded = true;
    return value;
  }

  /// Throws [SecureStorageFailure] when the platform keystore rejects the
  /// write.
  ///
  /// The in-memory cache is populated BEFORE the persist is attempted, and
  /// deliberately stays populated when it fails. The session is genuinely
  /// valid — the server issued this token and this process holds it — so the
  /// only thing a failed persist costs is survival across a cold start.
  /// Dropping the cache too would additionally break every request in the
  /// current process, turning a recoverable annoyance into a dead session.
  /// The caller learns about the failure from the thrown
  /// [SecureStorageFailure] and tells the user what it actually means.
  Future<void> writeToken(String token) async {
    _cachedToken = token;
    _tokenLoaded = true;
    await _safeWrite(SecureStorageKeys.bearerToken, token);
  }

  /// Clears the in-memory cache even when the platform delete fails, so a
  /// broken keystore can never leave a logged-out session still holding a
  /// usable token. Rethrows [SecureStorageFailure] for reporting; callers
  /// that must proceed regardless catch it.
  Future<void> clearToken() async {
    try {
      await _safeDelete(SecureStorageKeys.bearerToken);
    } finally {
      _cachedToken = null;
      _tokenLoaded = true;
    }
  }

  Future<String?> readLastOrgCode() =>
      _safeRead(SecureStorageKeys.lastOrgCode);

  Future<void> writeLastOrgCode(String orgCode) =>
      _safeWrite(SecureStorageKeys.lastOrgCode, orgCode);

  Future<void> clearLastOrgCode() =>
      _safeDelete(SecureStorageKeys.lastOrgCode);

  Future<String?> readApiBaseUrlOverride() =>
      _safeRead(SecureStorageKeys.apiBaseUrlOverride);

  Future<void> writeApiBaseUrlOverride(String url) =>
      _safeWrite(SecureStorageKeys.apiBaseUrlOverride, url);

  Future<void> clearApiBaseUrlOverride() =>
      _safeDelete(SecureStorageKeys.apiBaseUrlOverride);

  Future<bool> readBackgroundSyncTipSeen() async {
    final v = await _safeRead(SecureStorageKeys.backgroundSyncTipSeen);
    return v == 'true';
  }

  Future<void> markBackgroundSyncTipSeen() =>
      _safeWrite(SecureStorageKeys.backgroundSyncTipSeen, 'true');

  Future<DateTime?> readLocationTrackingLastCleanStop() async {
    final v = await _safeRead(SecureStorageKeys.locationTrackingLastCleanStop);
    if (v == null || v.isEmpty) return null;
    return DateTime.tryParse(v);
  }

  Future<void> writeLocationTrackingLastCleanStop(DateTime t) => _safeWrite(
        SecureStorageKeys.locationTrackingLastCleanStop,
        t.toIso8601String(),
      );

  Future<void> clearLocationTrackingLastCleanStop() =>
      _safeDelete(SecureStorageKeys.locationTrackingLastCleanStop);

  Future<bool> readLocationTrackingConsent(String appUserId) async {
    final v = await _safeRead(
      SecureStorageKeys.locationTrackingConsentKey(appUserId),
    );
    return v == 'true';
  }

  Future<void> writeLocationTrackingConsent(String appUserId) => _safeWrite(
        SecureStorageKeys.locationTrackingConsentKey(appUserId),
        'true',
      );

  Future<String?> readRecentCheckinLabels() =>
      _safeRead(SecureStorageKeys.recentCheckinLabels);

  Future<void> writeRecentCheckinLabels(String json) =>
      _safeWrite(SecureStorageKeys.recentCheckinLabels, json);

  Future<String?> readPrivacyUrlOverride() =>
      _safeRead(SecureStorageKeys.privacyUrlOverride);

  Future<void> writePrivacyUrlOverride(String url) =>
      _safeWrite(SecureStorageKeys.privacyUrlOverride, url);

  Future<void> clearPrivacyUrlOverride() =>
      _safeDelete(SecureStorageKeys.privacyUrlOverride);
}

/// Riverpod provider so consumers can `ref.read(secureStorageProvider)`.
final secureStorageProvider = Provider<SecureStorage>((ref) {
  return SecureStorage(null, ref.watch(errorReporterProvider));
});
