import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/storage/secure_storage.dart';

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
