import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';

/// Test double for `AuthNotifier`. Skips the bootstrap call entirely so
/// widget tests can pin the auth state to whatever they need.
class FakeAuthNotifier extends AuthNotifier {
  FakeAuthNotifier(this._initial);

  final AsyncValue<AuthState> _initial;

  Future<void> Function()? onLogin;
  Future<void> Function()? onLogout;
  Future<void> Function()? onChangePassword;
  Future<void> Function()? onRetry;

  @override
  Future<AuthState> build() async {
    final v = _initial;
    if (v is AsyncData<AuthState>) {
      return v.value;
    }
    if (v is AsyncError<AuthState>) {
      throw v.error;
    }
    return const AuthState.loading();
  }

  void setState(AsyncValue<AuthState> value) {
    state = value;
  }

  @override
  Future<void> login({
    required String orgCode,
    required String username,
    required String password,
  }) async {
    await onLogin?.call();
  }

  @override
  Future<void> logout() async {
    await onLogout?.call();
  }

  @override
  Future<void> changePassword({
    required String currentPassword,
    required String newPassword,
  }) async {
    await onChangePassword?.call();
  }

  @override
  Future<void> retry() async {
    await onRetry?.call();
  }

  @override
  Future<void> refreshMe() async {}
}
