import '../../../core/api/models/app_user.dart';
import '../../../core/api/models/org.dart';

/// Auth state machine. Sealed so `go_router`'s redirect can switch on cases
/// exhaustively and the compiler catches any new case.
///
/// Dart 3 sealed classes give us the same exhaustiveness as freezed unions
/// without the build_runner dance.
sealed class AuthState {
  const AuthState();

  /// App startup, or an in-flight `/app/me` after auto-login.
  const factory AuthState.loading() = AuthLoading;

  /// No token, or a 401 cleared the session. Default landing for the user.
  const factory AuthState.unauthenticated() = AuthUnauthenticated;

  /// Successful identity load. `needsPasswordChange` gates navigation to /.
  const factory AuthState.authenticated({
    required AppUser user,
    required Org org,
    required bool needsPasswordChange,
  }) = AuthAuthenticated;

  /// Recoverable failure (e.g. network) during bootstrap. UI shows a retry.
  const factory AuthState.error(String message) = AuthError;
}

class AuthLoading extends AuthState {
  const AuthLoading();

  @override
  bool operator ==(Object other) => other is AuthLoading;

  @override
  int get hashCode => 0;
}

class AuthUnauthenticated extends AuthState {
  const AuthUnauthenticated();

  @override
  bool operator ==(Object other) => other is AuthUnauthenticated;

  @override
  int get hashCode => 1;
}

class AuthAuthenticated extends AuthState {
  const AuthAuthenticated({
    required this.user,
    required this.org,
    required this.needsPasswordChange,
  });

  final AppUser user;
  final Org org;
  final bool needsPasswordChange;

  AuthAuthenticated copyWith({
    AppUser? user,
    Org? org,
    bool? needsPasswordChange,
  }) {
    return AuthAuthenticated(
      user: user ?? this.user,
      org: org ?? this.org,
      needsPasswordChange: needsPasswordChange ?? this.needsPasswordChange,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is AuthAuthenticated &&
        other.user == user &&
        other.org == org &&
        other.needsPasswordChange == needsPasswordChange;
  }

  @override
  int get hashCode => Object.hash(user, org, needsPasswordChange);
}

class AuthError extends AuthState {
  const AuthError(this.message);

  final String message;

  @override
  bool operator ==(Object other) =>
      other is AuthError && other.message == message;

  @override
  int get hashCode => message.hashCode;
}
