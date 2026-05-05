import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/api_error.dart';
import '../../../core/storage/secure_storage.dart';
import '../../checkin/data/checkin_queue_db.dart';
import '../../checkin/state/handover_notice_provider.dart';
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
      await _runHandoverWipe(me.user.id);
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
    } catch (e) {
      // Anything not an ApiException (e.g. JSON parse TypeError) would
      // otherwise propagate out of `build()` and leave the AsyncNotifier in
      // AsyncValue.error — the splash treats that as "loading" and the
      // router stays on /splash. Surface it as a recoverable AuthError so
      // the splash can render a retry button with the real message.
      return AuthState.error('解析失敗：$e');
    }
  }

  /// Filter the device-local queue to the currently authenticated user.
  /// Rows whose `app_user_id` doesn't match are deleted; on a non-zero count,
  /// the home screen's SnackBar listener picks up the notice via
  /// `pendingHandoverNoticeProvider`.
  Future<void> _runHandoverWipe(String currentUserId) async {
    try {
      final db = ref.read(checkinQueueDbProvider);
      final deleted = await db.wipeForOtherUsers(currentUserId);
      if (deleted > 0) {
        ref.read(pendingHandoverNoticeProvider.notifier).state =
            '前個帳號的 $deleted 筆未送事件已清除';
      }
    } catch (_) {
      // Wipe failures shouldn't block login. The next tick can't submit
      // mismatched rows anyway because the bearer token won't match.
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
    // Wait for `build()` (auto-login `_bootstrap`) to settle before mutating
    // state. Otherwise the build Future can resolve mid-login and overwrite
    // our `data(authenticated)` with the bootstrap result, leaving the user
    // stranded on /splash with a stale GET /app/me request fired off.
    await future;
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
      // Run AFTER state flip so home is mounted and its listener can pick up
      // the toast notice. Yielding via microtask gives go_router a chance to
      // navigate before we publish the message.
      // ignore: unawaited_futures
      Future<void>.microtask(() => _runHandoverWipe(res.user.id));
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
    await future;
    final storage = ref.read(secureStorageProvider);
    try {
      final repo = await ref.read(authRepositoryProvider.future);
      await repo.logout();
    } on ApiException {
      // ignore — logout is best-effort.
    }
    await storage.clearToken();
    // KEEP last_org_code on logout: per spec, /login pre-fills the org_code
    // field on subsequent visits — including after logout — so the user only
    // has to retype username + password.
    state = const AsyncValue<AuthState>.data(AuthState.unauthenticated());
  }

  /// `POST /app/me/password`. On success refreshes the auth state via /me
  /// so `needs_password_change` is updated. On `INVALID_PASSWORD` rethrows
  /// so the screen can render the friendly Chinese message.
  Future<void> changePassword({
    required String currentPassword,
    required String newPassword,
  }) async {
    await future;
    final repo = await ref.read(authRepositoryProvider.future);
    await repo.changePassword(
      currentPassword: currentPassword,
      newPassword: newPassword,
    );
    state = AsyncValue<AuthState>.data(await _fetchMe());
  }

  /// Best-effort refetch of `/app/me` without re-running the handover wipe.
  /// Used by the home screen's app-resume hook to keep cached `Org` settings
  /// (especially `transfer_enabled`) and `AppUser` fields fresh while the
  /// session is unchanged. Resume is NOT a login event, so `wipeForOtherUsers`
  /// must NOT run from this path — that would risk wiping the current user's
  /// own queue if a transient `/me` failure had returned a stale id earlier.
  ///
  /// On `401` we clear the session as if a regular `/me` had failed auth.
  /// On any other failure we leave the cached state untouched.
  Future<void> refreshMe() async {
    await future;
    final current = state.value;
    if (current is! AuthAuthenticated) return;
    try {
      final repo = await ref.read(authRepositoryProvider.future);
      final me = await repo.me();
      state = AsyncValue<AuthState>.data(
        AuthState.authenticated(
          user: me.user,
          org: me.org,
          needsPasswordChange: me.needsPasswordChange,
        ),
      );
    } on ApiException catch (e) {
      if (_isAuthFailure(e)) {
        final storage = ref.read(secureStorageProvider);
        await storage.clearToken();
        state = const AsyncValue<AuthState>.data(AuthState.unauthenticated());
      }
      // Other errors: leave cached state alone, user can retry by
      // backgrounding+foregrounding again.
    } catch (_) {
      // Best-effort — a parse error on a refresh shouldn't blow away the
      // logged-in shell.
    }
  }
}

final authProvider = AsyncNotifierProvider<AuthNotifier, AuthState>(
  AuthNotifier.new,
);
