/// AppUser status. Mirrors `AppUserStatus` in `api/src/domain.rs`. Wire format
/// is lowercase strings.
enum AppUserStatus {
  active,
  disabled;

  String toJson() => name;

  static AppUserStatus fromJson(String wire) {
    switch (wire) {
      case 'active':
        return AppUserStatus.active;
      case 'disabled':
        return AppUserStatus.disabled;
      default:
        throw ArgumentError.value(
          wire,
          'wire',
          'Unknown AppUserStatus value',
        );
    }
  }
}

/// AppUser DTO mirroring `AppUserDto` in `api/src/handlers/app_dto.rs`.
///
/// Hand-rolled value class (immutable + value equality + JSON conversion).
/// We started with freezed + json_serializable but the build_runner
/// orchestration is friction-y in this Claude Code setup; rewriting the
/// five DTOs by hand was both shorter and clearer. A future
/// `add-openapi-codegen` change will replace these anyway.
class AppUser {
  const AppUser({
    required this.id,
    required this.username,
    required this.displayName,
    required this.status,
    required this.needsPasswordChange,
    required this.createdAt,
    this.lastLoginAt,
  });

  final String id;
  final String username;
  final String displayName;
  final AppUserStatus status;
  final bool needsPasswordChange;
  final String? lastLoginAt;
  final String createdAt;

  factory AppUser.fromJson(Map<String, dynamic> json) {
    return AppUser(
      id: json['id'] as String,
      username: json['username'] as String,
      displayName: json['display_name'] as String,
      status: AppUserStatus.fromJson(json['status'] as String),
      needsPasswordChange: json['needs_password_change'] as bool,
      lastLoginAt: json['last_login_at'] as String?,
      createdAt: json['created_at'] as String,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'username': username,
        'display_name': displayName,
        'status': status.toJson(),
        'needs_password_change': needsPasswordChange,
        if (lastLoginAt != null) 'last_login_at': lastLoginAt,
        'created_at': createdAt,
      };

  AppUser copyWith({
    String? id,
    String? username,
    String? displayName,
    AppUserStatus? status,
    bool? needsPasswordChange,
    String? lastLoginAt,
    String? createdAt,
  }) {
    return AppUser(
      id: id ?? this.id,
      username: username ?? this.username,
      displayName: displayName ?? this.displayName,
      status: status ?? this.status,
      needsPasswordChange: needsPasswordChange ?? this.needsPasswordChange,
      lastLoginAt: lastLoginAt ?? this.lastLoginAt,
      createdAt: createdAt ?? this.createdAt,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is AppUser &&
        other.id == id &&
        other.username == username &&
        other.displayName == displayName &&
        other.status == status &&
        other.needsPasswordChange == needsPasswordChange &&
        other.lastLoginAt == lastLoginAt &&
        other.createdAt == createdAt;
  }

  @override
  int get hashCode => Object.hash(
        id,
        username,
        displayName,
        status,
        needsPasswordChange,
        lastLoginAt,
        createdAt,
      );

  @override
  String toString() =>
      'AppUser(id: $id, username: $username, displayName: $displayName, '
      'status: ${status.name}, needsPasswordChange: $needsPasswordChange)';
}
