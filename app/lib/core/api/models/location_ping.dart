/// AppUser-side payload for `POST /app/checkin/locations`. Hand-rolled per
/// the same pattern as `Org` / `CheckinEventDto`.
class LocationPingInput {
  const LocationPingInput({
    required this.lat,
    required this.lng,
    this.accuracy,
    required this.occurredAtClient,
  });

  final double lat;
  final double lng;
  final double? accuracy;
  final String occurredAtClient;

  Map<String, dynamic> toJson() => <String, dynamic>{
        'lat': lat,
        'lng': lng,
        if (accuracy != null) 'accuracy': accuracy,
        'occurred_at_client': occurredAtClient,
      };
}

class SubmitLocationPingsRequest {
  const SubmitLocationPingsRequest({required this.pings});

  final List<LocationPingInput> pings;

  Map<String, dynamic> toJson() => <String, dynamic>{
        'pings': pings.map((p) => p.toJson()).toList(growable: false),
      };
}

class RejectedPingDto {
  const RejectedPingDto({
    required this.index,
    required this.code,
    required this.message,
  });

  final int index;
  final String code;
  final String message;

  factory RejectedPingDto.fromJson(Map<String, dynamic> json) =>
      RejectedPingDto(
        index: (json['index'] as num).toInt(),
        code: json['code'] as String,
        message: json['message'] as String? ?? '',
      );
}

class SubmitLocationPingsResponse {
  const SubmitLocationPingsResponse({
    required this.acceptedCount,
    required this.rejected,
  });

  final int acceptedCount;
  final List<RejectedPingDto> rejected;

  factory SubmitLocationPingsResponse.fromJson(Map<String, dynamic> json) =>
      SubmitLocationPingsResponse(
        acceptedCount: (json['accepted_count'] as num).toInt(),
        rejected: (json['rejected'] as List<dynamic>? ?? const <dynamic>[])
            .map((r) => RejectedPingDto.fromJson(r as Map<String, dynamic>))
            .toList(growable: false),
      );
}

/// Admin-side ping shape returned by `GET /checkin/users/:id/locations`.
/// Not strictly needed by the AppUser app today but the field set is in
/// scope for the change and useful for future debug screens.
class LocationPingDto {
  const LocationPingDto({
    required this.id,
    required this.appUserId,
    required this.lat,
    required this.lng,
    this.accuracyMeters,
    required this.occurredAtClient,
    required this.occurredAtServer,
  });

  final String id;
  final String appUserId;
  final double lat;
  final double lng;
  final double? accuracyMeters;
  final String occurredAtClient;
  final String occurredAtServer;

  factory LocationPingDto.fromJson(Map<String, dynamic> json) =>
      LocationPingDto(
        id: json['id'] as String,
        appUserId: json['app_user_id'] as String,
        lat: (json['lat'] as num).toDouble(),
        lng: (json['lng'] as num).toDouble(),
        accuracyMeters: (json['accuracy_meters'] as num?)?.toDouble(),
        occurredAtClient: json['occurred_at_client'] as String,
        occurredAtServer: json['occurred_at_server'] as String,
      );
}
