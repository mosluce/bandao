/// Mirrors `CheckinEventType` in `api/src/domain/checkin.rs`. Wire format is
/// snake_case strings: `clock_in / clock_out / transfer_out / transfer_in`.
enum CheckinEventType {
  clockIn,
  clockOut,
  transferOut,
  transferIn;

  String toJson() {
    switch (this) {
      case CheckinEventType.clockIn:
        return 'clock_in';
      case CheckinEventType.clockOut:
        return 'clock_out';
      case CheckinEventType.transferOut:
        return 'transfer_out';
      case CheckinEventType.transferIn:
        return 'transfer_in';
    }
  }

  static CheckinEventType fromJson(String wire) {
    switch (wire) {
      case 'clock_in':
        return CheckinEventType.clockIn;
      case 'clock_out':
        return CheckinEventType.clockOut;
      case 'transfer_out':
        return CheckinEventType.transferOut;
      case 'transfer_in':
        return CheckinEventType.transferIn;
      default:
        throw ArgumentError.value(
          wire,
          'wire',
          'Unknown CheckinEventType value',
        );
    }
  }
}

/// Mirrors `EventSource`. Wire: `app | admin_force`.
enum EventSource {
  app,
  adminForce;

  String toJson() => this == EventSource.app ? 'app' : 'admin_force';

  static EventSource fromJson(String wire) {
    switch (wire) {
      case 'app':
        return EventSource.app;
      case 'admin_force':
        return EventSource.adminForce;
      default:
        throw ArgumentError.value(wire, 'wire', 'Unknown EventSource value');
    }
  }
}

/// Mirrors `EventInitiatorKind`. Wire: `app_user | dashboard_user`.
enum EventInitiatorKind {
  appUser,
  dashboardUser;

  String toJson() =>
      this == EventInitiatorKind.appUser ? 'app_user' : 'dashboard_user';

  static EventInitiatorKind fromJson(String wire) {
    switch (wire) {
      case 'app_user':
        return EventInitiatorKind.appUser;
      case 'dashboard_user':
        return EventInitiatorKind.dashboardUser;
      default:
        throw ArgumentError.value(
          wire,
          'wire',
          'Unknown EventInitiatorKind value',
        );
    }
  }
}

class GeoPoint {
  const GeoPoint({required this.lat, required this.lng});

  final double lat;
  final double lng;

  factory GeoPoint.fromJson(Map<String, dynamic> json) => GeoPoint(
        lat: (json['lat'] as num).toDouble(),
        lng: (json['lng'] as num).toDouble(),
      );

  Map<String, dynamic> toJson() => <String, dynamic>{'lat': lat, 'lng': lng};

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is GeoPoint && other.lat == lat && other.lng == lng;

  @override
  int get hashCode => Object.hash(lat, lng);

  @override
  String toString() => 'GeoPoint($lat, $lng)';
}

class EventLocation {
  const EventLocation({
    required this.coordinates,
    this.accuracyMeters,
    this.regionName,
    this.manualLabel,
  });

  final GeoPoint coordinates;
  final double? accuracyMeters;
  final String? regionName;
  final String? manualLabel;

  factory EventLocation.fromJson(Map<String, dynamic> json) => EventLocation(
        coordinates:
            GeoPoint.fromJson(json['coordinates'] as Map<String, dynamic>),
        accuracyMeters: (json['accuracy_meters'] as num?)?.toDouble(),
        regionName: json['region_name'] as String?,
        manualLabel: json['manual_label'] as String?,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'coordinates': coordinates.toJson(),
        if (accuracyMeters != null) 'accuracy_meters': accuracyMeters,
        if (regionName != null) 'region_name': regionName,
        if (manualLabel != null) 'manual_label': manualLabel,
      };

  EventLocation copyWith({
    GeoPoint? coordinates,
    double? accuracyMeters,
    String? regionName,
    String? manualLabel,
  }) =>
      EventLocation(
        coordinates: coordinates ?? this.coordinates,
        accuracyMeters: accuracyMeters ?? this.accuracyMeters,
        regionName: regionName ?? this.regionName,
        manualLabel: manualLabel ?? this.manualLabel,
      );

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is EventLocation &&
        other.coordinates == coordinates &&
        other.accuracyMeters == accuracyMeters &&
        other.regionName == regionName &&
        other.manualLabel == manualLabel;
  }

  @override
  int get hashCode =>
      Object.hash(coordinates, accuracyMeters, regionName, manualLabel);
}

/// Mirrors `CheckinEventDto`.
class CheckinEventDto {
  const CheckinEventDto({
    required this.id,
    required this.appUserId,
    required this.eventType,
    required this.occurredAtClient,
    required this.occurredAtServer,
    required this.source,
    required this.initiatedByKind,
    required this.initiatedById,
    required this.location,
    this.reason,
    required this.hasSkewWarning,
  });

  final String id;
  final String appUserId;
  final CheckinEventType eventType;
  final String occurredAtClient;
  final String occurredAtServer;
  final EventSource source;
  final EventInitiatorKind initiatedByKind;
  final String initiatedById;
  final EventLocation location;
  final String? reason;
  final bool hasSkewWarning;

  factory CheckinEventDto.fromJson(Map<String, dynamic> json) =>
      CheckinEventDto(
        id: json['id'] as String,
        appUserId: json['app_user_id'] as String,
        eventType: CheckinEventType.fromJson(json['event_type'] as String),
        occurredAtClient: json['occurred_at_client'] as String,
        occurredAtServer: json['occurred_at_server'] as String,
        source: EventSource.fromJson(json['source'] as String),
        initiatedByKind:
            EventInitiatorKind.fromJson(json['initiated_by_kind'] as String),
        initiatedById: json['initiated_by_id'] as String,
        location:
            EventLocation.fromJson(json['location'] as Map<String, dynamic>),
        reason: json['reason'] as String?,
        hasSkewWarning: json['has_skew_warning'] as bool? ?? false,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'app_user_id': appUserId,
        'event_type': eventType.toJson(),
        'occurred_at_client': occurredAtClient,
        'occurred_at_server': occurredAtServer,
        'source': source.toJson(),
        'initiated_by_kind': initiatedByKind.toJson(),
        'initiated_by_id': initiatedById,
        'location': location.toJson(),
        if (reason != null) 'reason': reason,
        'has_skew_warning': hasSkewWarning,
      };

  CheckinEventDto copyWith({
    String? id,
    String? appUserId,
    CheckinEventType? eventType,
    String? occurredAtClient,
    String? occurredAtServer,
    EventSource? source,
    EventInitiatorKind? initiatedByKind,
    String? initiatedById,
    EventLocation? location,
    String? reason,
    bool? hasSkewWarning,
  }) =>
      CheckinEventDto(
        id: id ?? this.id,
        appUserId: appUserId ?? this.appUserId,
        eventType: eventType ?? this.eventType,
        occurredAtClient: occurredAtClient ?? this.occurredAtClient,
        occurredAtServer: occurredAtServer ?? this.occurredAtServer,
        source: source ?? this.source,
        initiatedByKind: initiatedByKind ?? this.initiatedByKind,
        initiatedById: initiatedById ?? this.initiatedById,
        location: location ?? this.location,
        reason: reason ?? this.reason,
        hasSkewWarning: hasSkewWarning ?? this.hasSkewWarning,
      );

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is CheckinEventDto &&
        other.id == id &&
        other.appUserId == appUserId &&
        other.eventType == eventType &&
        other.occurredAtClient == occurredAtClient &&
        other.occurredAtServer == occurredAtServer &&
        other.source == source &&
        other.initiatedByKind == initiatedByKind &&
        other.initiatedById == initiatedById &&
        other.location == location &&
        other.reason == reason &&
        other.hasSkewWarning == hasSkewWarning;
  }

  @override
  int get hashCode => Object.hash(
        id,
        appUserId,
        eventType,
        occurredAtClient,
        occurredAtServer,
        source,
        initiatedByKind,
        initiatedById,
        location,
        reason,
        hasSkewWarning,
      );
}
