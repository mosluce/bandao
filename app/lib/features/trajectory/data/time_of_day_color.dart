/// Shared time-of-day trajectory color scale.
///
/// Maps a local wall-clock time to a color on a two-pole warm→cool ramp:
/// `06:00` warmest → `22:00` coolest, clamped outside that window. The ramp
/// runs through the red–purple side (never green/rainbow) and stays chromatic
/// throughout so the path is legible on the light CARTO Positron basemap.
///
/// This is a contract shared verbatim with admin-web
/// (`admin-web/utils/timeOfDayColor.ts`) — the anchors, domain, clamp, and
/// linear-RGB interpolation MUST match so both surfaces render identically.
/// See openspec capability `app-personal-trajectory`.
library;

import 'dart:ui';

/// `(minuteOfDay, color)` anchors. Minute of day = hour * 60 + minute.
const List<(int, Color)> _anchors = <(int, Color)>[
  (6 * 60, Color(0xFFEA580C)), // 06:00 orange (warmest)
  (10 * 60, Color(0xFFE11D48)), // 10:00 rose
  (14 * 60, Color(0xFFC026D3)), // 14:00 fuchsia (warm↔cool bridge)
  (18 * 60, Color(0xFF7C3AED)), // 18:00 violet
  (22 * 60, Color(0xFF4338CA)), // 22:00 indigo (coolest)
];

/// Color for a local [time] on the scale. Only the wall-clock hour/minute of
/// [time] is used; the date is ignored.
Color timeOfDayColor(DateTime time) {
  final minute = time.hour * 60 + time.minute;
  return timeOfDayColorForMinute(minute);
}

/// Color for a [minuteOfDay] (`hour * 60 + minute`), clamped to the domain.
Color timeOfDayColorForMinute(int minuteOfDay) {
  final first = _anchors.first;
  final last = _anchors.last;
  if (minuteOfDay <= first.$1) return first.$2;
  if (minuteOfDay >= last.$1) return last.$2;

  for (var i = 0; i < _anchors.length - 1; i++) {
    final (loM, loC) = _anchors[i];
    final (hiM, hiC) = _anchors[i + 1];
    if (minuteOfDay >= loM && minuteOfDay <= hiM) {
      final t = (minuteOfDay - loM) / (hiM - loM);
      return Color.lerp(loC, hiC, t)!;
    }
  }
  return last.$2; // unreachable (clamped above)
}
