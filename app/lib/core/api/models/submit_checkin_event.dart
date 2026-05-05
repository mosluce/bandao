import 'checkin_event.dart';
import 'checkin_status.dart';

class SubmitCheckinEventRequest {
  const SubmitCheckinEventRequest({
    required this.eventType,
    required this.lat,
    required this.lng,
    this.accuracy,
    this.manualLabel,
    required this.occurredAtClient,
  });

  final CheckinEventType eventType;
  final double lat;
  final double lng;
  final double? accuracy;
  final String? manualLabel;
  final String occurredAtClient;

  Map<String, dynamic> toJson() => <String, dynamic>{
        'event_type': eventType.toJson(),
        'lat': lat,
        'lng': lng,
        if (accuracy != null) 'accuracy': accuracy,
        if (manualLabel != null) 'manual_label': manualLabel,
        'occurred_at_client': occurredAtClient,
      };
}

class SubmitCheckinEventResponse {
  const SubmitCheckinEventResponse({required this.event, required this.status});

  final CheckinEventDto event;
  final CheckinUserStatusDto status;

  factory SubmitCheckinEventResponse.fromJson(Map<String, dynamic> json) =>
      SubmitCheckinEventResponse(
        event: CheckinEventDto.fromJson(json['event'] as Map<String, dynamic>),
        status: CheckinUserStatusDto.fromJson(
          json['status'] as Map<String, dynamic>,
        ),
      );
}

/// RFC3339 timestamp from the device wall clock, with the local zone offset
/// suffix (e.g. `2026-05-04T18:00:12.345+08:00`). The server parses to UTC.
String nowOccurredAtClient([DateTime? now]) {
  final dt = now ?? DateTime.now();
  final iso = dt.toIso8601String();
  if (dt.isUtc) return iso;
  final off = dt.timeZoneOffset;
  final sign = off.isNegative ? '-' : '+';
  final hh = off.inHours.abs().toString().padLeft(2, '0');
  final mm = (off.inMinutes.abs() % 60).toString().padLeft(2, '0');
  return '$iso$sign$hh:$mm';
}
