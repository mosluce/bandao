/// Org checkin sub-document. Mirrors `OrgCheckinDto` in
/// `api/src/handlers/auth.rs`.
class OrgCheckin {
  const OrgCheckin({
    required this.transferEnabled,
    this.locationTrackingEnabled = false,
  });

  final bool transferEnabled;
  final bool locationTrackingEnabled;

  factory OrgCheckin.fromJson(Map<String, dynamic> json) => OrgCheckin(
        transferEnabled: json['transfer_enabled'] as bool,
        locationTrackingEnabled:
            json['location_tracking_enabled'] as bool? ?? false,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'transfer_enabled': transferEnabled,
        'location_tracking_enabled': locationTrackingEnabled,
      };

  OrgCheckin copyWith({
    bool? transferEnabled,
    bool? locationTrackingEnabled,
  }) =>
      OrgCheckin(
        transferEnabled: transferEnabled ?? this.transferEnabled,
        locationTrackingEnabled:
            locationTrackingEnabled ?? this.locationTrackingEnabled,
      );

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is OrgCheckin &&
          other.transferEnabled == transferEnabled &&
          other.locationTrackingEnabled == locationTrackingEnabled;

  @override
  int get hashCode => Object.hash(transferEnabled, locationTrackingEnabled);

  @override
  String toString() =>
      'OrgCheckin(transfer: $transferEnabled, locationTracking: $locationTrackingEnabled)';
}

/// Org DTO mirroring `OrgDto` in `api/src/handlers/auth.rs`.
///
/// Includes `timezone` and the `checkin` sub-document — both are present on
/// main and future checkin features depend on them.
class Org {
  const Org({
    required this.id,
    required this.name,
    required this.code,
    required this.ownerId,
    required this.timezone,
    required this.checkin,
    this.slug,
    this.slugChangedAt,
  });

  final String id;
  final String name;
  final String code;
  final String ownerId;
  final String timezone;
  final OrgCheckin checkin;
  final String? slug;
  final String? slugChangedAt;

  factory Org.fromJson(Map<String, dynamic> json) {
    return Org(
      id: json['id'] as String,
      name: json['name'] as String,
      code: json['code'] as String,
      ownerId: json['owner_id'] as String,
      timezone: json['timezone'] as String,
      checkin: OrgCheckin.fromJson(json['checkin'] as Map<String, dynamic>),
      slug: json['slug'] as String?,
      slugChangedAt: json['slug_changed_at'] as String?,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'name': name,
        'code': code,
        'owner_id': ownerId,
        'timezone': timezone,
        'checkin': checkin.toJson(),
        if (slug != null) 'slug': slug,
        if (slugChangedAt != null) 'slug_changed_at': slugChangedAt,
      };

  Org copyWith({
    String? id,
    String? name,
    String? code,
    String? ownerId,
    String? timezone,
    OrgCheckin? checkin,
    String? slug,
    String? slugChangedAt,
  }) {
    return Org(
      id: id ?? this.id,
      name: name ?? this.name,
      code: code ?? this.code,
      ownerId: ownerId ?? this.ownerId,
      timezone: timezone ?? this.timezone,
      checkin: checkin ?? this.checkin,
      slug: slug ?? this.slug,
      slugChangedAt: slugChangedAt ?? this.slugChangedAt,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is Org &&
        other.id == id &&
        other.name == name &&
        other.code == code &&
        other.ownerId == ownerId &&
        other.timezone == timezone &&
        other.checkin == checkin &&
        other.slug == slug &&
        other.slugChangedAt == slugChangedAt;
  }

  @override
  int get hashCode => Object.hash(
        id,
        name,
        code,
        ownerId,
        timezone,
        checkin,
        slug,
        slugChangedAt,
      );

  @override
  String toString() =>
      'Org(id: $id, name: $name, code: $code, timezone: $timezone)';
}
