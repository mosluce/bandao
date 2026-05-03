import 'app_user.dart';
import 'org.dart';

/// `POST /app/auth/login` response. Mirrors `AppLoginResponse` in
/// `api/src/handlers/app_dto.rs`.
class LoginResponse {
  const LoginResponse({
    required this.token,
    required this.expiresAt,
    required this.user,
    required this.org,
    required this.needsPasswordChange,
  });

  final String token;
  final String expiresAt;
  final AppUser user;
  final Org org;
  final bool needsPasswordChange;

  factory LoginResponse.fromJson(Map<String, dynamic> json) => LoginResponse(
        token: json['token'] as String,
        expiresAt: json['expires_at'] as String,
        user: AppUser.fromJson(json['user'] as Map<String, dynamic>),
        org: Org.fromJson(json['org'] as Map<String, dynamic>),
        needsPasswordChange: json['needs_password_change'] as bool,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'token': token,
        'expires_at': expiresAt,
        'user': user.toJson(),
        'org': org.toJson(),
        'needs_password_change': needsPasswordChange,
      };

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is LoginResponse &&
        other.token == token &&
        other.expiresAt == expiresAt &&
        other.user == user &&
        other.org == org &&
        other.needsPasswordChange == needsPasswordChange;
  }

  @override
  int get hashCode =>
      Object.hash(token, expiresAt, user, org, needsPasswordChange);

  @override
  String toString() => 'LoginResponse(user: $user, org: $org)';
}

/// `GET /app/me` response. Mirrors `AppMeResponse` in
/// `api/src/handlers/app_dto.rs`.
class MeResponse {
  const MeResponse({
    required this.user,
    required this.org,
    required this.needsPasswordChange,
  });

  final AppUser user;
  final Org org;
  final bool needsPasswordChange;

  factory MeResponse.fromJson(Map<String, dynamic> json) => MeResponse(
        user: AppUser.fromJson(json['user'] as Map<String, dynamic>),
        org: Org.fromJson(json['org'] as Map<String, dynamic>),
        needsPasswordChange: json['needs_password_change'] as bool,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'user': user.toJson(),
        'org': org.toJson(),
        'needs_password_change': needsPasswordChange,
      };

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is MeResponse &&
        other.user == user &&
        other.org == org &&
        other.needsPasswordChange == needsPasswordChange;
  }

  @override
  int get hashCode => Object.hash(user, org, needsPasswordChange);

  @override
  String toString() => 'MeResponse(user: $user, org: $org)';
}
