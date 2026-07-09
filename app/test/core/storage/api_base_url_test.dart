import 'package:bandao_app/core/env/env.dart';
import 'package:bandao_app/core/storage/api_base_url.dart';
import 'package:bandao_app/core/storage/server_url_override.dart';
import 'package:flutter_test/flutter_test.dart';

import '../../helpers/fake_secure_storage.dart';

void main() {
  group('validateBaseUrlOverride (release)', () {
    test('accepts https url with host', () {
      expect(
        validateBaseUrlOverride('https://api.myco.com', release: true),
        isNull,
      );
    });

    test('rejects http', () {
      expect(
        validateBaseUrlOverride('http://api.myco.com', release: true),
        BaseUrlOverrideError.insecureScheme,
      );
    });

    test('rejects http localhost', () {
      expect(
        validateBaseUrlOverride('http://localhost:9090', release: true),
        BaseUrlOverrideError.insecureScheme,
      );
    });

    test('rejects value with no scheme', () {
      expect(
        validateBaseUrlOverride('api.myco.com', release: true),
        BaseUrlOverrideError.malformed,
      );
    });

    test('rejects path-only value', () {
      expect(
        validateBaseUrlOverride('/app/auth', release: true),
        BaseUrlOverrideError.malformed,
      );
    });
  });

  group('validateBaseUrlOverride (debug)', () {
    test('accepts http localhost', () {
      expect(
        validateBaseUrlOverride('http://localhost:9090', release: false),
        isNull,
      );
    });

    test('accepts LAN IP over http', () {
      expect(
        validateBaseUrlOverride('http://192.168.1.42:9090', release: false),
        isNull,
      );
    });

    test('accepts https', () {
      expect(
        validateBaseUrlOverride('https://api.myco.com', release: false),
        isNull,
      );
    });

    test('still rejects malformed', () {
      expect(
        validateBaseUrlOverride('nonsense', release: false),
        BaseUrlOverrideError.malformed,
      );
    });
  });

  group('ApiBaseUrlResolver', () {
    test('returns the compile-time default when no override is stored',
        () async {
      final resolver = ApiBaseUrlResolver(
        ServerUrlOverride(FakeSecureStorage()),
      );
      expect(await resolver.effectiveBaseUrl(), Env.compileTimeDefault());
    });

    test('returns the override when present', () async {
      final resolver = ApiBaseUrlResolver(
        ServerUrlOverride(
          FakeSecureStorage(apiBaseUrlOverride: 'https://api.myco.com'),
        ),
      );
      expect(await resolver.effectiveBaseUrl(), 'https://api.myco.com');
    });
  });
}
