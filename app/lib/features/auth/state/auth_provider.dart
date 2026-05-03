import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_error.dart';
import '../../../core/storage/secure_storage.dart';
import '../data/auth_repository.dart';
import 'auth_state.dart';

/// AuthNotifier owns the auth state machine. Construction kicks off
/// `_bootstrap()` which executes the auto-login flow:
///
/// 1. Read `auth.bearer_token` from secure storage.
/// 2. Absent  -> AuthState.unauthenticated.
/// 3. Present -> GET /app/me.
///    - 200: AuthState.authenticated(user, org, needs_password_change).
///    - 401 (UNAUTHORIZED / INVALID_CREDENTIALS): clear token,
///      AuthState.unauthenticated.
///    - Network error: AuthState.error(...) — token is NOT cleared so the
///      user can retry.
class AuthNotifier extends AsyncNotifier<AuthState> {
  @override
  Future<AuthState> build() async {
    return _bootstrap();
  }

  Future<AuthState> _bootstrap() async {
    final storage = ref.read(secureStorageProvider);
    final token = await storage.readToken();
    if (token == null || token.isEmpty) {
      return const AuthState.unauthenticated();
    }
    return _fetchMe();
  }

  Future<AuthState> _fetchMe() async {
    final storage = ref.read(secureStorageProvider);
    try {
      final repo = await ref.read(authRepositoryProvider.future);
      final me = await repo.me();
      return AuthState.authenticated(
        user: me.user,
        org: me.org,
        needsPasswordChange: me.needsPasswordChange,
      );
    } on ApiException catch (e) {
      if (_isAuthFailure(e)) {
        await storage.clearToken();
        return const AuthState.unauthenticated();
      }
      // Network errors / unknowns: keep the token, surface error so UI
      // can show a retry button.
      return AuthState.error(e.message);
    }
  }

  bool _isAuthFailure(ApiException e) {
    if (e.status == 401) return true;
    return e.code == ApiErrorCode.unauthorized ||
        e.code == ApiErrorCode.invalidCredentials;
  }

  /// Re-runs the auto-login flow — used by the splash retry button.
  Future<void> retry() async {
    state = const AsyncValue<AuthState>.data(AuthState.loading());
    state = await AsyncValue.guard<AuthState>(_bootstrap);
  }

  /// `POST /app/auth/login`. On 423 NEEDS_PASSWORD_CHANGE we still want a
  /// successful login flow — but the API only returns 200 with the flag set,
  /// so a 423 from this endpoint is unusual; we treat it as a successful
  /// authenticated state with the flag forced on.
  Future<void> login({
    required String orgCode,
    required String username,
    required String password,
  }) async {
    state = const AsyncValue<AuthState>.data(AuthState.loading());
    final storage = ref.read(secureStorageProvider);
    try {
      final repo = await ref.read(authRepositoryProvider.future);
      final res = await repo.login(
        orgCode: orgCode,
        username: username,
        password: password,
      );
      await storage.writeToken(res.token);
      await storage.writeLastOrgCode(orgCode);
      state = AsyncValue<AuthState>.data(
        AuthState.authenticated(
          user: res.user,
          org: res.org,
          needsPasswordChange: res.needsPasswordChange,
        ),
      );
    } on ApiException catch (e, st) {
      // Login failures must not pollute the auth state with an "error" case
      // that the redirect would interpret as "not on /login". Stay on
      // unauthenticated and let the screen surface `e` via its caller.
      state = const AsyncValue<AuthState>.data(AuthState.unauthenticated());
      Error.throwWithStackTrace(e, st);
    }
  }

  /// Best-effort logout: always clear local state regardless of network.
  Future<void> logout() async {
    final storage = ref.read(secureStorageProvider);
    try {
      final repo = await ref.read(authRepositoryProvider.future);
      await repo.logout();
    } on ApiException {
      // ignore — logout is best-effort.
    }
    await storage.clearToken();
    await storage.clearLastOrgCode();
    state = const AsyncValue<AuthState>.data(AuthState.unauthenticated());
  }

  /// `POST /app/me/password`. On success refreshes the auth state via /me
  /// so `needs_password_change` is updated. On `INVALID_PASSWORD` rethrows
  /// so the screen can render the friendly Chinese message.
  Future<void> changePassword({
    required String currentPassword,
    required String newPassword,
  }) async {
    final repo = await ref.read(authRepositoryProvider.future);
    await repo.changePassword(
      currentPassword: currentPassword,
      newPassword: newPassword,
    );
    state = AsyncValue<AuthState>.data(await _fetchMe());
  }
}

final authProvider = AsyncNotifierProvider<AuthNotifier, AuthState>(
  AuthNotifier.new,
);
