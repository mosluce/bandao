import 'checkin_event.dart';

/// Mirrors `AppUserCheckinStatus` in `api/src/domain/checkin.rs`. Wire format is
/// snake_case: `off_duty | on_site | in_transit`.
enum AppUserCheckinStatus {
  offDuty,
  onSite,
  inTransit;

  String toJson() {
    switch (this) {
      case AppUserCheckinStatus.offDuty:
        return 'off_duty';
      case AppUserCheckinStatus.onSite:
        return 'on_site';
      case AppUserCheckinStatus.inTransit:
        return 'in_transit';
    }
  }

  static AppUserCheckinStatus fromJson(String wire) {
    switch (wire) {
      case 'off_duty':
        return AppUserCheckinStatus.offDuty;
      case 'on_site':
        return AppUserCheckinStatus.onSite;
      case 'in_transit':
        return AppUserCheckinStatus.inTransit;
      default:
        throw ArgumentError.value(
          wire,
          'wire',
          'Unknown AppUserCheckinStatus value',
        );
    }
  }
}

class CheckinUserStatusDto {
  const CheckinUserStatusDto({
    required this.appUserId,
    required this.status,
    this.currentShiftStartedAt,
    this.lastEvent,
    required this.hasSkewWarning,
  });

  final String appUserId;
  final AppUserCheckinStatus status;
  final String? currentShiftStartedAt;
  final CheckinEventDto? lastEvent;
  final bool hasSkewWarning;

  factory CheckinUserStatusDto.fromJson(Map<String, dynamic> json) =>
      CheckinUserStatusDto(
        appUserId: json['app_user_id'] as String,
        status: AppUserCheckinStatus.fromJson(json['status'] as String),
        currentShiftStartedAt: json['current_shift_started_at'] as String?,
        lastEvent: json['last_event'] == null
            ? null
            : CheckinEventDto.fromJson(
                json['last_event'] as Map<String, dynamic>,
              ),
        hasSkewWarning: json['has_skew_warning'] as bool? ?? false,
      );

  Map<String, dynamic> toJson() => <String, dynamic>{
        'app_user_id': appUserId,
        'status': status.toJson(),
        if (currentShiftStartedAt != null)
          'current_shift_started_at': currentShiftStartedAt,
        if (lastEvent != null) 'last_event': lastEvent!.toJson(),
        'has_skew_warning': hasSkewWarning,
      };

  CheckinUserStatusDto copyWith({
    String? appUserId,
    AppUserCheckinStatus? status,
    String? currentShiftStartedAt,
    CheckinEventDto? lastEvent,
    bool? hasSkewWarning,
  }) =>
      CheckinUserStatusDto(
        appUserId: appUserId ?? this.appUserId,
        status: status ?? this.status,
        currentShiftStartedAt:
            currentShiftStartedAt ?? this.currentShiftStartedAt,
        lastEvent: lastEvent ?? this.lastEvent,
        hasSkewWarning: hasSkewWarning ?? this.hasSkewWarning,
      );

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is CheckinUserStatusDto &&
        other.appUserId == appUserId &&
        other.status == status &&
        other.currentShiftStartedAt == currentShiftStartedAt &&
        other.lastEvent == lastEvent &&
        other.hasSkewWarning == hasSkewWarning;
  }

  @override
  int get hashCode => Object.hash(
        appUserId,
        status,
        currentShiftStartedAt,
        lastEvent,
        hasSkewWarning,
      );
}
