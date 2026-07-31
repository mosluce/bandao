import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/features/checkin/presentation/history_screen.dart';

void main() {
  group('formatLocationLine', () {
    test('shows what the worker wrote, then where the server placed them', () {
      expect(
        formatLocationLine(
          manualLabel: '甲工地',
          regionName: '高雄市鳳山區頂庄路',
          lat: 22.6116,
          lng: 120.3006,
        ),
        '甲工地, 高雄市鳳山區頂庄路',
      );
    });

    test('label alone when the region is missing', () {
      // Reverse geocoding is fail-soft, so region can legitimately be null.
      expect(
        formatLocationLine(
          manualLabel: '甲工地',
          regionName: null,
          lat: 22.6116,
          lng: 120.3006,
        ),
        '甲工地',
      );
    });

    test('region alone for events recorded before labels existed', () {
      expect(
        formatLocationLine(
          manualLabel: null,
          regionName: '高雄市鳳山區頂庄路',
          lat: 22.6116,
          lng: 120.3006,
        ),
        '高雄市鳳山區頂庄路',
      );
    });

    test('coordinates when neither exists', () {
      expect(
        formatLocationLine(
          manualLabel: null,
          regionName: null,
          lat: 22.6116,
          lng: 120.3006,
        ),
        '22.6116, 120.3006',
      );
    });

    test('blank strings count as absent, not as content', () {
      expect(
        formatLocationLine(
          manualLabel: '   ',
          regionName: '',
          lat: 22.6116,
          lng: 120.3006,
        ),
        '22.6116, 120.3006',
      );
    });

    test('a pending local row shows its label before the region arrives', () {
      // The worker typed the label on this device; the server has not
      // geocoded the event yet, so the region follows after sync.
      expect(
        formatLocationLine(
          manualLabel: '乙工地',
          regionName: null,
          lat: 22.6116,
          lng: 120.3006,
        ),
        '乙工地',
      );
    });
  });
}
