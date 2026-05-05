import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/features/checkin/data/location_tracking_service.dart';
import 'package:argus_app/features/checkin/presentation/tracking_chip.dart';
import 'package:argus_app/features/checkin/state/location_tracking_controller.dart';
import 'package:argus_app/l10n/app_localizations.dart';

void main() {
  group('TrackingChip', () {
    testWidgets('hidden when controller reports not running', (tester) async {
      await _pump(tester, isRunning: false);
      expect(find.text('定位追蹤中'), findsNothing);
      expect(find.byIcon(Icons.my_location), findsNothing);
    });

    testWidgets('visible with MM:SS elapsed when running', (tester) async {
      final startedAt = DateTime.now().subtract(const Duration(minutes: 1, seconds: 5));
      await _pump(tester, isRunning: true, startedAt: startedAt);
      expect(find.byIcon(Icons.my_location), findsOneWidget);
      expect(find.textContaining('定位追蹤中'), findsOneWidget);
      expect(find.textContaining('01:0'), findsOneWidget);
    });

    testWidgets('subscribes to tickStream while visible', (tester) async {
      final ticks = StreamController<DateTime>.broadcast();
      addTearDown(ticks.close);
      final svc = _FakeTrackingService(
        active: true,
        startedAt: DateTime.now().subtract(const Duration(seconds: 30)),
        ticks: ticks.stream,
      );
      await _pump(tester, isRunning: true, service: svc);

      // Emit a tick — must not crash and the chip should still render.
      ticks.add(DateTime.now());
      await tester.pump();
      expect(find.byIcon(Icons.my_location), findsOneWidget);
      expect(ticks.hasListener, isTrue, reason: 'StreamBuilder subscribed');
    });
  });
}

Future<void> _pump(
  WidgetTester tester, {
  required bool isRunning,
  DateTime? startedAt,
  _FakeTrackingService? service,
}) async {
  final svc = service ??
      _FakeTrackingService(
        active: isRunning,
        startedAt: startedAt,
        ticks: const Stream<DateTime>.empty(),
      );
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        locationTrackingServiceProvider.overrideWithValue(svc),
        locationTrackingControllerProvider
            .overrideWith(() => _FakeController(isRunning)),
      ],
      child: MaterialApp(
        home: const Scaffold(body: TrackingChip()),
        locale: const Locale('zh', 'TW'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
      ),
    ),
  );
  await tester.pump();
}

class _FakeController extends LocationTrackingController {
  _FakeController(this._initial);
  final bool _initial;
  @override
  bool build() => _initial;
}

class _FakeTrackingService implements LocationTrackingService {
  _FakeTrackingService({
    required this.active,
    required this.startedAt,
    required this.ticks,
  });

  bool active;
  @override
  DateTime? startedAt;
  final Stream<DateTime> ticks;

  @override
  bool get isActive => active;

  @override
  Stream<DateTime> get tickStream => ticks;

  @override
  Future<void> start({required String appUserId}) async {
    active = true;
  }

  @override
  Future<void> stop() async {
    active = false;
  }
}
