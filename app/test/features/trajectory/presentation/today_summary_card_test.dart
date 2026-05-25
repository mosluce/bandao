import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';

import 'package:bandao_app/core/api/models/checkin_status.dart';
import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/checkin/data/location_tracking_service.dart';
import 'package:bandao_app/features/checkin/state/checkin_status_provider.dart';
import 'package:bandao_app/features/trajectory/data/my_locations_repository.dart';
import 'package:bandao_app/features/trajectory/presentation/today_summary_card.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

class _StubRepository implements MyLocationsRepository {
  _StubRepository(this._pings);
  final List<LocationPingDto> _pings;

  @override
  Future<List<LocationPingDto>> listForRange({
    required DateTime from,
    required DateTime to,
    int? limit,
  }) async =>
      _pings;
}

class _StubStatusNotifier extends CheckinStatusNotifier {
  _StubStatusNotifier(this._value);
  final CheckinUserStatusDto? _value;
  @override
  Future<CheckinUserStatusDto?> build() async => _value;
}

class _FakeTrackingService implements LocationTrackingService {
  final StreamController<DateTime> _ctl = StreamController<DateTime>.broadcast();

  @override
  Stream<DateTime> get tickStream => _ctl.stream;

  @override
  bool get isActive => false;

  @override
  DateTime? get startedAt => null;

  @override
  noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName} not stubbed');
}

CheckinUserStatusDto _status(AppUserCheckinStatus s) => CheckinUserStatusDto(
      appUserId: 'u',
      status: s,
      currentShiftStartedAt: null,
      lastEvent: null,
      hasSkewWarning: false,
    );

LocationPingDto _ping(String iso) => LocationPingDto(
      id: 'x',
      appUserId: 'u',
      lat: 25.0,
      lng: 121.0,
      occurredAtClient: iso,
      occurredAtServer: iso,
    );

Future<void> _pump(
  WidgetTester tester, {
  required CheckinUserStatusDto? status,
  required List<LocationPingDto> pings,
  GoRouter? router,
}) async {
  final theRouter = router ??
      GoRouter(
        initialLocation: '/',
        routes: [
          GoRoute(
            path: '/',
            builder: (c, s) => const Scaffold(body: TodaySummaryCard()),
          ),
          GoRoute(
            path: '/trajectory',
            builder: (c, s) => const Scaffold(body: Text('TRAJ-ROUTE')),
          ),
        ],
      );
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        myLocationsRepositoryProvider
            .overrideWith((ref) async => _StubRepository(pings)),
        checkinStatusProvider
            .overrideWith(() => _StubStatusNotifier(status)),
        locationTrackingServiceProvider
            .overrideWithValue(_FakeTrackingService()),
      ],
      child: MaterialApp.router(
        locale: const Locale('zh', 'TW'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        routerConfig: theRouter,
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pump();
}

void main() {
  group('TodaySummaryCard', () {
    testWidgets('hidden when off-shift and zero pings', (tester) async {
      await _pump(
        tester,
        status: _status(AppUserCheckinStatus.offDuty),
        pings: const [],
      );
      expect(find.byKey(const ValueKey('todaySummaryCard')), findsNothing);
    });

    testWidgets('visible when on-shift even with zero pings', (tester) async {
      await _pump(
        tester,
        status: _status(AppUserCheckinStatus.onSite),
        pings: const [],
      );
      expect(find.byKey(const ValueKey('todaySummaryCard')), findsOneWidget);
      expect(find.text('我的今天'), findsOneWidget);
    });

    testWidgets('visible off-shift if today has pings', (tester) async {
      final iso = DateTime.now().toUtc().toIso8601String();
      await _pump(
        tester,
        status: _status(AppUserCheckinStatus.offDuty),
        pings: [_ping(iso), _ping(iso)],
      );
      expect(find.byKey(const ValueKey('todaySummaryCard')), findsOneWidget);
    });

    testWidgets('tap navigates to /trajectory', (tester) async {
      await _pump(
        tester,
        status: _status(AppUserCheckinStatus.onSite),
        pings: const [],
      );
      await tester.tap(find.byKey(const ValueKey('todaySummaryCard')));
      await tester.pumpAndSettle();
      expect(find.text('TRAJ-ROUTE'), findsOneWidget);
    });
  });
}
