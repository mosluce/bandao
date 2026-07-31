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
    required this.displayName,
    required this.status,
    required this.needsPasswordChange,
    required this.createdAt,
    this.username,
    this.externalKey,
    this.lastLoginAt,
  });

  final String id;

  /// Set for internal AppUsers; `null` for external shadow users, which are
  /// identified by [externalKey] instead. The server omits whichever one does
  /// not apply (`skip_serializing_if` on both), so exactly one is present.
  ///
  /// This was `required String` until it was found to throw a `TypeError` on
  /// every external login — the model predates `external-db-auth` and was
  /// never updated for shadow users.
  final String? username;

  /// Set for external shadow users — their identifier in the customer's own
  /// system. `null` for internal AppUsers.
  final String? externalKey;

  final String displayName;
  final AppUserStatus status;
  final bool needsPasswordChange;
  final String? lastLoginAt;
  final String createdAt;

  factory AppUser.fromJson(Map<String, dynamic> json) {
    return AppUser(
      id: json['id'] as String,
      username: json['username'] as String?,
      externalKey: json['external_key'] as String?,
      displayName: json['display_name'] as String,
      status: AppUserStatus.fromJson(json['status'] as String),
      needsPasswordChange: json['needs_password_change'] as bool,
      lastLoginAt: json['last_login_at'] as String?,
      createdAt: json['created_at'] as String,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        if (username != null) 'username': username,
        if (externalKey != null) 'external_key': externalKey,
        'display_name': displayName,
        'status': status.toJson(),
        'needs_password_change': needsPasswordChange,
        if (lastLoginAt != null) 'last_login_at': lastLoginAt,
        'created_at': createdAt,
      };

  AppUser copyWith({
    String? id,
    String? username,
    String? externalKey,
    String? displayName,
    AppUserStatus? status,
    bool? needsPasswordChange,
    String? lastLoginAt,
    String? createdAt,
  }) {
    return AppUser(
      id: id ?? this.id,
      username: username ?? this.username,
      externalKey: externalKey ?? this.externalKey,
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
        other.externalKey == externalKey &&
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
        externalKey,
        displayName,
        status,
        needsPasswordChange,
        lastLoginAt,
        createdAt,
      );

  /// What to show as the machine-readable identity: the internal username, or
  /// the external key for shadow users. `null` when the server sent neither,
  /// which the callers render as nothing rather than as an empty line.
  String? get identityLabel => username ?? externalKey;

  @override
  String toString() =>
      'AppUser(id: $id, username: $username, externalKey: $externalKey, '
      'displayName: $displayName, '
      'status: ${status.name}, needsPasswordChange: $needsPasswordChange)';
}
