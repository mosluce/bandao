import 'package:drift/native.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/api_error.dart';
import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/auth_responses.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/core/storage/secure_storage.dart';
import 'package:argus_app/features/auth/data/auth_repository.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/features/checkin/data/checkin_queue_db.dart';

void main() {
  group('AuthNotifier auto-login', () {
    test('no token -> unauthenticated', () async {
      final state = await _bootstrap(
        storage: _FakeSecureStorage(),
        repo: _FakeRepo(),
      );
      expect(state, isA<AuthUnauthenticated>());
    });

    test('valid token + 200 /me -> authenticated', () async {
      final state = await _bootstrap(
        storage: _FakeSecureStorage(token: 'abc'),
        repo: _FakeRepo(meResponse: _meOk),
      );
      expect(state, isA<AuthAuthenticated>());
      final authed = state as AuthAuthenticated;
      expect(authed.user.username, 'alice');
      expect(authed.needsPasswordChange, false);
    });

    test('401 -> token cleared, unauthenticated', () async {
      final storage = _FakeSecureStorage(token: 'abc');
      final state = await _bootstrap(
        storage: storage,
        repo: _FakeRepo(meThrow: ApiException.unauthorized()),
      );
      expect(state, isA<AuthUnauthenticated>());
      expect(await storage.readToken(), isNull);
    });

    test('network error -> error state, token preserved', () async {
      final storage = _FakeSecureStorage(token: 'abc');
      final state = await _bootstrap(
        storage: storage,
        repo: _FakeRepo(meThrow: ApiException.network('boom')),
      );
      expect(state, isA<AuthError>());
      expect(await storage.readToken(), 'abc');
    });
  });

  group('AuthNotifier login()', () {
    test('success persists token + org_code', () async {
      final storage = _FakeSecureStorage();
      final repo = _FakeRepo(loginResponse: _loginOk);
      final container = _container(storage: storage, repo: repo);

      final notifier = container.read(authProvider.notifier);
      await container.read(authProvider.future); // wait for bootstrap.
      await notifier.login(
        orgCode: 'ABCDEFGHIJ',
        username: 'alice',
        password: 'pass1234',
      );

      expect(await storage.readToken(), 'tok');
      expect(await storage.readLastOrgCode(), 'ABCDEFGHIJ');
      final state = container.read(authProvider).value;
      expect(state, isA<AuthAuthenticated>());
    });

    test('INVALID_CREDENTIALS rethrows; state stays unauthenticated',
        () async {
      final storage = _FakeSecureStorage();
      final repo = _FakeRepo(loginThrow: ApiException.invalidCredentials());
      final container = _container(storage: storage, repo: repo);

      final notifier = container.read(authProvider.notifier);
      await container.read(authProvider.future);

      await expectLater(
        () => notifier.login(
          orgCode: 'X',
          username: 'alice',
          password: 'wrong',
        ),
        throwsA(isA<ApiException>()),
      );
      expect(await storage.readToken(), isNull);
      expect(container.read(authProvider).value, isA<AuthUnauthenticated>());
    });
  });

  group('AuthNotifier logout()', () {
    test('clears token but preserves org_code on success', () async {
      final storage = _FakeSecureStorage(token: 'abc', orgCode: 'C');
      final repo = _FakeRepo(meResponse: _meOk);
      final container = _container(storage: storage, repo: repo);

      await container.read(authProvider.future);
      await container.read(authProvider.notifier).logout();

      expect(await storage.readToken(), isNull);
      // Per spec: org_code is preserved across logout so /login pre-fills.
      expect(await storage.readLastOrgCode(), 'C');
      expect(container.read(authProvider).value, isA<AuthUnauthenticated>());
    });

    test('clears token but preserves org_code on network failure', () async {
      final storage = _FakeSecureStorage(token: 'abc', orgCode: 'C');
      final repo = _FakeRepo(
        meResponse: _meOk,
        logoutThrow: ApiException.network(),
      );
      final container = _container(storage: storage, repo: repo);

      await container.read(authProvider.future);
      await container.read(authProvider.notifier).logout();

      expect(await storage.readToken(), isNull);
      expect(await storage.readLastOrgCode(), 'C');
      expect(container.read(authProvider).value, isA<AuthUnauthenticated>());
    });
  });
}

ProviderContainer _container({
  required _FakeSecureStorage storage,
  required _FakeRepo repo,
}) {
  final db = CheckinQueueDb.forTesting(NativeDatabase.memory());
  final container = ProviderContainer(
    overrides: <Override>[
      secureStorageProvider.overrideWithValue(storage),
      authRepositoryProvider.overrideWith((ref) async => repo),
      checkinQueueDbProvider.overrideWithValue(db),
    ],
  );
  addTearDown(() async {
    container.dispose();
    await db.close();
  });
  return container;
}

Future<AuthState> _bootstrap({
  required _FakeSecureStorage storage,
  required _FakeRepo repo,
}) async {
  final container = _container(storage: storage, repo: repo);
  return container.read(authProvider.future);
}

// ----- fakes -----

class _FakeSecureStorage implements SecureStorage {
  _FakeSecureStorage({String? token, String? orgCode, String? override})
      : _token = token,
        _orgCode = orgCode,
        _override = override;

  String? _token;
  String? _orgCode;
  String? _override;

  @override
  Future<String?> readToken() async => _token;

  @override
  Future<void> writeToken(String token) async => _token = token;

  @override
  Future<void> clearToken() async => _token = null;

  @override
  Future<String?> readLastOrgCode() async => _orgCode;

  @override
  Future<void> writeLastOrgCode(String orgCode) async => _orgCode = orgCode;

  @override
  Future<void> clearLastOrgCode() async => _orgCode = null;

  @override
  Future<String?> readApiBaseUrlOverride() async => _override;

  @override
  Future<void> writeApiBaseUrlOverride(String url) async => _override = url;

  @override
  Future<void> clearApiBaseUrlOverride() async => _override = null;

  @override
  Future<bool> readBackgroundSyncTipSeen() async => false;

  @override
  Future<void> markBackgroundSyncTipSeen() async {}

  @override
  Future<DateTime?> readLocationTrackingLastCleanStop() async => null;

  @override
  Future<void> writeLocationTrackingLastCleanStop(DateTime t) async {}

  @override
  Future<void> clearLocationTrackingLastCleanStop() async {}

  @override
  Future<bool> readLocationTrackingConsent(String appUserId) async => false;

  @override
  Future<void> writeLocationTrackingConsent(String appUserId) async {}

  @override
  Future<String?> readPrivacyUrlOverride() async => null;

  @override
  Future<void> writePrivacyUrlOverride(String url) async {}

  @override
  Future<void> clearPrivacyUrlOverride() async {}
}

class _FakeRepo implements AuthRepository {
  _FakeRepo({
    this.loginResponse,
    this.meResponse,
    this.loginThrow,
    this.meThrow,
    this.logoutThrow,
  });

  LoginResponse? loginResponse;
  MeResponse? meResponse;
  ApiException? loginThrow;
  ApiException? meThrow;
  ApiException? logoutThrow;

  @override
  Future<LoginResponse> login({
    required String orgCode,
    required String username,
    required String password,
  }) async {
    if (loginThrow != null) throw loginThrow!;
    return loginResponse!;
  }

  @override
  Future<MeResponse> me() async {
    if (meThrow != null) throw meThrow!;
    return meResponse!;
  }

  @override
  Future<void> logout() async {
    if (logoutThrow != null) throw logoutThrow!;
  }

  @override
  Future<void> changePassword({
    required String currentPassword,
    required String newPassword,
  }) async {}
}

// ----- fixtures -----

const _user = AppUser(
  id: 'u1',
  username: 'alice',
  displayName: 'Alice',
  status: AppUserStatus.active,
  needsPasswordChange: false,
  createdAt: '2025-01-01T00:00:00Z',
);

const _org = Org(
  id: 'o1',
  name: 'Acme',
  code: 'ABCDEFGHIJ',
  ownerId: 'u1',
  timezone: 'Asia/Taipei',
  checkin: OrgCheckin(transferEnabled: true),
);

const _meOk = MeResponse(user: _user, org: _org, needsPasswordChange: false);

const _loginOk = LoginResponse(
  token: 'tok',
  expiresAt: '2025-12-31T00:00:00Z',
  user: _user,
  org: _org,
  needsPasswordChange: false,
);
