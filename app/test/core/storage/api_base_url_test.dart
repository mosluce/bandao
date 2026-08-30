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

  group('validateBaseUrlOverride (empty string)', () {
    // The empty string stays malformed in BOTH modes and deliberately carries
    // no "clear the override" meaning. Callers that want to clear detect a
    // blank input and take the clear path before validating, which keeps this
    // a pure predicate over candidate URLs rather than something every future
    // caller has to know has a second mode.
    test('is malformed in release', () {
      expect(
        validateBaseUrlOverride('', release: true),
        BaseUrlOverrideError.malformed,
      );
    });

    test('is malformed in debug', () {
      expect(
        validateBaseUrlOverride('', release: false),
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
