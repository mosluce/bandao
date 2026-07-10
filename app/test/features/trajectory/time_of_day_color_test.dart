import 'dart:ui';

import 'package:bandao_app/features/trajectory/data/time_of_day_color.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('timeOfDayColorForMinute', () {
    test('anchors map to their exact colors', () {
      expect(timeOfDayColorForMinute(6 * 60), const Color(0xFFEA580C));
      expect(timeOfDayColorForMinute(14 * 60), const Color(0xFFC026D3));
      expect(timeOfDayColorForMinute(22 * 60), const Color(0xFF4338CA));
    });

    test('clamps below 06:00 and above 22:00', () {
      expect(timeOfDayColorForMinute(5 * 60 + 30), const Color(0xFFEA580C));
      expect(timeOfDayColorForMinute(0), const Color(0xFFEA580C));
      expect(timeOfDayColorForMinute(23 * 60 + 15), const Color(0xFF4338CA));
      expect(timeOfDayColorForMinute(24 * 60), const Color(0xFF4338CA));
    });

    test('interpolates between two anchors (08:00 is midway 06:00–10:00)', () {
      final c = timeOfDayColorForMinute(8 * 60);
      final expected = Color.lerp(
        const Color(0xFFEA580C),
        const Color(0xFFE11D48),
        0.5,
      )!;
      expect(c.toARGB32(), expected.toARGB32());
    });

    test('is monotonic-ish: distinct anchors give distinct colors', () {
      final colors = [6, 10, 14, 18, 22]
          .map((h) => timeOfDayColorForMinute(h * 60).toARGB32())
          .toSet();
      expect(colors.length, 5);
    });
  });

  group('timeOfDayColor(DateTime)', () {
    test('uses wall-clock hour/minute, ignores date', () {
      final a = timeOfDayColor(DateTime(2026, 1, 1, 10, 0));
      final b = timeOfDayColor(DateTime(2020, 12, 31, 10, 0));
      expect(a.toARGB32(), b.toARGB32());
      expect(a.toARGB32(), const Color(0xFFE11D48).toARGB32());
    });
  });
}
