import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/app_user.dart';

/// The payloads below are the two shapes `AppUserDto` actually serialises.
/// Both `username` and `external_key` carry `skip_serializing_if`, so exactly
/// one key is present and the other is **absent entirely** — not null.
Map<String, dynamic> _internalUserJson() => <String, dynamic>{
      'id': 'u1',
      'auth_source': 'internal',
      'username': 'alice',
      'display_name': 'Alice',
      'status': 'active',
      'needs_password_change': false,
      'is_locked': false,
      'created_at': '2026-01-01T00:00:00Z',
    };

Map<String, dynamic> _externalUserJson() => <String, dynamic>{
      'id': 'u2',
      'auth_source': 'external',
      'external_key': 'Allen',
      'display_name': '陳先生',
      'status': 'active',
      'needs_password_change': false,
      'is_locked': false,
      'created_at': '2026-01-01T00:00:00Z',
    };

void main() {
  group('AppUser.fromJson', () {
    test('parses an internal user', () {
      final u = AppUser.fromJson(_internalUserJson());

      expect(u.username, 'alice');
      expect(u.externalKey, isNull);
      expect(u.identityLabel, 'alice');
    });

    // The regression. `username` was a hard `as String` cast, so every
    // external login threw a TypeError while parsing the login response —
    // after the request had already returned 200. Since all of an
    // external-auth Org's AppUsers are shadow users, that Org could never log
    // in at all.
    test('parses an external shadow user, which has no username', () {
      final u = AppUser.fromJson(_externalUserJson());

      expect(u.username, isNull);
      expect(u.externalKey, 'Allen');
      expect(u.displayName, '陳先生');
    });

    test('an external user falls back to the external key for identity', () {
      expect(AppUser.fromJson(_externalUserJson()).identityLabel, 'Allen');
    });

    test('identity is null when the server sends neither identifier', () {
      final json = _externalUserJson()..remove('external_key');

      // Callers render nothing rather than an empty line.
      expect(AppUser.fromJson(json).identityLabel, isNull);
    });

    test('round-trips through toJson without inventing absent keys', () {
      final external = AppUser.fromJson(_externalUserJson());
      final json = external.toJson();

      expect(json.containsKey('username'), isFalse);
      expect(json['external_key'], 'Allen');
      expect(AppUser.fromJson(json), external);
    });
  });
}
