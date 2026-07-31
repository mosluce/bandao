import 'package:flutter/services.dart' show PlatformException;
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/storage/secure_storage.dart';
import 'package:bandao_app/core/telemetry/error_reporter.dart';

/// Captures what the storage wrapper reports, so tests can assert that a
/// recovered failure is still made visible rather than silently absorbed.
class _RecordingReporter extends ErrorReporter {
  final List<Object> errors = <Object>[];
  final List<Map<String, Object?>> contexts = <Map<String, Object?>>[];

  @override
  void recordNonFatal(
    Object error,
    StackTrace stackTrace, {
    String? reason,
    Map<String, Object?> context = const <String, Object?>{},
  }) {
    errors.add(error);
    contexts.add(context);
  }
}

/// Counting fake — extends `FlutterSecureStorage` so we satisfy the type
/// argument to `SecureStorage`'s constructor without faking the whole
/// plugin platform interface. Only `read` / `write` / `delete` are
/// overridden; other methods (which we don't call from `SecureStorage`'s
/// token paths) inherit the real implementation, so any accidental
/// regression that calls them in tests will surface a missing-platform
/// error rather than silently passing.
class _CountingStorage extends FlutterSecureStorage {
  _CountingStorage({this.initialValue}) : super();

  String? initialValue;
  bool throwOnRead = false;
  bool throwPlatformExceptionOnRead = false;
  bool throwPlatformExceptionOnWrite = false;
  bool throwPlatformExceptionOnDelete = false;
  int reads = 0;
  int writes = 0;
  int deletes = 0;

  @override
  Future<String?> read({
    required String key,
    IOSOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    MacOsOptions? mOptions,
    WindowsOptions? wOptions,
  }) async {
    reads++;
    if (throwOnRead) {
      throw StateError(
        'underlying storage read should not be reached after the cache is populated',
      );
    }
    if (throwPlatformExceptionOnRead) {
      // Mirrors the Android Keystore BadPaddingException surface: the
      // plugin wraps the native decrypt failure in a PlatformException.
      throw PlatformException(code: 'read_error', message: 'BAD_DECRYPT');
    }
    return initialValue;
  }

  @override
  Future<void> write({
    required String key,
    required String? value,
    IOSOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    MacOsOptions? mOptions,
    WindowsOptions? wOptions,
  }) async {
    writes++;
    if (throwPlatformExceptionOnWrite) {
      throw PlatformException(code: 'write_error', message: 'KEYCHAIN_WRITE');
    }
    initialValue = value;
  }

  @override
  Future<void> delete({
    required String key,
    IOSOptions? iOptions,
    AndroidOptions? aOptions,
    LinuxOptions? lOptions,
    WebOptions? webOptions,
    MacOsOptions? mOptions,
    WindowsOptions? wOptions,
  }) async {
    deletes++;
    if (throwPlatformExceptionOnDelete) {
      throw PlatformException(code: 'delete_error', message: 'KEYCHAIN_DELETE');
    }
    initialValue = null;
  }
}

void main() {
  group('SecureStorage bearer token cache', () {
    test('readToken hits underlying storage only once', () async {
      final fake = _CountingStorage(initialValue: 'tok-abc');
      final storage = SecureStorage(fake);

      expect(await storage.readToken(), 'tok-abc');
      expect(await storage.readToken(), 'tok-abc');
      expect(await storage.readToken(), 'tok-abc');

      expect(fake.reads, 1, reason: 'second and third reads must hit cache');
    });

    test('writeToken populates cache; subsequent reads do not hit storage', () async {
      final fake = _CountingStorage(initialValue: null);
      final storage = SecureStorage(fake);

      await storage.writeToken('abc');
      // Arm the fake so a subsequent read would throw — proving the value
      // returned next must come from the cache rather than the underlying
      // storage.
      fake.throwOnRead = true;

      expect(await storage.readToken(), 'abc');
      expect(fake.writes, 1);
      expect(
        fake.reads,
        0,
        reason: 'cache should serve readToken after writeToken',
      );
    });

    test('clearToken empties cache; subsequent reads return null without storage hit',
        () async {
      final fake = _CountingStorage(initialValue: 'tok-abc');
      final storage = SecureStorage(fake);

      // Prime the cache by reading once.
      expect(await storage.readToken(), 'tok-abc');
      expect(fake.reads, 1);

      await storage.clearToken();
      expect(fake.deletes, 1);

      // Now arm the fake so the next read would throw — and confirm we
      // get null from the cache without ever touching storage.
      fake.throwOnRead = true;
      expect(await storage.readToken(), isNull);
      expect(fake.reads, 1, reason: 'no extra read after clearToken');
    });

    test('writeToken overwrites previously cached value', () async {
      final fake = _CountingStorage(initialValue: 'old');
      final storage = SecureStorage(fake);

      expect(await storage.readToken(), 'old');
      await storage.writeToken('new');

      // Cache should now serve 'new' — even if storage would say something
      // different (it shouldn't, but we want the cache to be the source
      // of truth post-write).
      fake.throwOnRead = true;
      expect(await storage.readToken(), 'new');
    });
  });

  group('SecureStorage undecryptable entry recovery', () {
    test('readToken returns null and deletes the corrupted entry instead of throwing',
        () async {
      final fake = _CountingStorage(initialValue: 'garbled-ciphertext')
        ..throwPlatformExceptionOnRead = true;
      final storage = SecureStorage(fake);

      expect(await storage.readToken(), isNull);
      expect(fake.deletes, 1);
    });

    test('readLastOrgCode returns null and deletes the corrupted entry instead of throwing',
        () async {
      final fake = _CountingStorage(initialValue: 'garbled-ciphertext')
        ..throwPlatformExceptionOnRead = true;
      final storage = SecureStorage(fake);

      expect(await storage.readLastOrgCode(), isNull);
      expect(fake.deletes, 1);
    });
  });

  group('SecureStorage write and delete failures', () {
    test('a failing token write throws a typed failure naming the key', () async {
      final fake = _CountingStorage()..throwPlatformExceptionOnWrite = true;
      final storage = SecureStorage(fake);

      await expectLater(
        storage.writeToken('abc'),
        throwsA(
          isA<SecureStorageFailure>()
              .having((f) => f.key, 'key', 'auth.bearer_token')
              .having(
                (f) => f.operation,
                'operation',
                SecureStorageOperation.write,
              ),
        ),
      );
    });

    test('no raw PlatformException escapes the wrapper', () async {
      final fake = _CountingStorage()..throwPlatformExceptionOnWrite = true;
      final storage = SecureStorage(fake);

      await expectLater(
        storage.writeToken('abc'),
        throwsA(isNot(isA<PlatformException>())),
      );
    });

    // The load-bearing one. A token that cannot be persisted is still a valid
    // session for this process — dropping the cache too would break every
    // outbound request and turn a recoverable annoyance into a dead session.
    test('a failing token write still populates the in-memory cache', () async {
      final fake = _CountingStorage()..throwPlatformExceptionOnWrite = true;
      final storage = SecureStorage(fake);

      await expectLater(storage.writeToken('abc'), throwsA(anything));

      fake.throwOnRead = true; // prove the value comes from the cache
      expect(await storage.readToken(), 'abc');
    });

    test('a failing write is reported', () async {
      final reporter = _RecordingReporter();
      final fake = _CountingStorage()..throwPlatformExceptionOnWrite = true;
      final storage = SecureStorage(fake, reporter);

      await expectLater(storage.writeToken('abc'), throwsA(anything));

      expect(reporter.errors.single, isA<SecureStorageFailure>());
      expect(
        reporter.contexts.single['secure_storage_key'],
        'auth.bearer_token',
      );
      expect(reporter.contexts.single['secure_storage_operation'], 'write');
    });

    test('a failing delete still clears the cached token', () async {
      final fake = _CountingStorage(initialValue: 'tok');
      final storage = SecureStorage(fake);
      expect(await storage.readToken(), 'tok');

      fake.throwPlatformExceptionOnDelete = true;
      await expectLater(
        storage.clearToken(),
        throwsA(isA<SecureStorageFailure>()),
      );

      // A broken keystore must never leave a logged-out session still
      // holding a usable token.
      expect(await storage.readToken(), isNull);
    });

    test('a failing delete is reported', () async {
      final reporter = _RecordingReporter();
      final fake = _CountingStorage()..throwPlatformExceptionOnDelete = true;
      final storage = SecureStorage(fake, reporter);

      await expectLater(storage.clearToken(), throwsA(anything));

      expect(reporter.errors.single, isA<SecureStorageFailure>());
      expect(reporter.contexts.single['secure_storage_operation'], 'delete');
    });
  });

  group('SecureStorage read failures are fail-soft but visible', () {
    test('an unreadable entry resolves as absent AND is reported', () async {
      final reporter = _RecordingReporter();
      final fake = _CountingStorage()..throwPlatformExceptionOnRead = true;
      final storage = SecureStorage(fake, reporter);

      expect(await storage.readToken(), isNull);
      expect(
        reporter.errors.single,
        isA<SecureStorageFailure>().having(
          (f) => f.operation,
          'operation',
          SecureStorageOperation.read,
        ),
        reason: 'silently absorbing this is what hid the real bug',
      );
    });

    test('a read whose corrupted-entry cleanup also fails still resolves absent',
        () async {
      final fake = _CountingStorage()
        ..throwPlatformExceptionOnRead = true
        ..throwPlatformExceptionOnDelete = true;
      final storage = SecureStorage(fake);

      expect(await storage.readToken(), isNull);
    });
  });

  group('SecureStorage iOS Keychain options', () {
    test('default storage uses KeychainAccessibility.first_unlock', () {
      expect(
        SecureStorage.defaultIosOptionsForTest.toMap()['accessibility'],
        'first_unlock',
        reason:
            'Bearer token must be readable while the device is locked after the first '
            'post-reboot unlock; otherwise background HTTP from a locked screen '
            'cannot attach the Authorization header.',
      );
    });
  });
}
