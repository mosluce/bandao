import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';

import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/checkin/data/geolocation_service.dart';
import 'package:bandao_app/features/trajectory/data/my_locations_repository.dart';
import 'package:bandao_app/features/trajectory/presentation/trajectory_screen.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

class _FakeGeolocationService implements GeolocationService {
  _FakeGeolocationService(this.permission);
  final LocationPermission permission;

  @override
  Future<LocationPermission> currentPermission() async => permission;

  @override
  Future<LocationPermission> requestPermission() async => permission;

  @override
  noSuchMethod(Invocation invocation) =>
      throw UnimplementedError('${invocation.memberName} not stubbed');
}

class _StubRepository implements MyLocationsRepository {
  _StubRepository(this._pings);
  final List<LocationPingDto> _pings;
  int callCount = 0;

  @override
  Future<List<LocationPingDto>> listForRange({
    required DateTime from,
    required DateTime to,
    int? limit,
  }) async {
    callCount += 1;
    return _pings;
  }
}

Future<void> _pump(
  WidgetTester tester, {
  required LocationPermission permission,
  required List<LocationPingDto> pings,
  _StubRepository? repoOut,
  bool settle = true,
}) async {
  final repo = repoOut ?? _StubRepository(pings);
  await tester.pumpWidget(
    ProviderScope(
      overrides: [
        geolocationServiceProvider
            .overrideWithValue(_FakeGeolocationService(permission)),
        myLocationsRepositoryProvider.overrideWith((ref) async => repo),
      ],
      child: const MaterialApp(
        locale: Locale('zh', 'TW'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: <LocalizationsDelegate<Object>>[
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        home: TrajectoryScreen(),
      ),
    ),
  );
  if (settle) {
    await tester.pumpAndSettle();
  } else {
    // Resolve the AsyncNotifier microtask without entering tile-fetch
    // settlement (FlutterMap would otherwise hit the test HttpClient
    // and bubble status-400 exceptions into the test result).
    await tester.pump();
    await tester.pump();
  }
}

void main() {
  group('TrajectoryScreen', () {
    // The "with-data" path mounts FlutterMap, which fetches network tiles
    // and TestWidgetsFlutterBinding always returns 400 → uncaught exception.
    // The data branch is covered indirectly:
    //   - TrajectoryController test verifies pings → stats computation
    //   - §11 smoke verifies the map+stats render on a real device.
    // The widget tests below stick to branches that don't mount the map.

    testWidgets('empty-day path renders the empty text and no map', (tester) async {
      await _pump(tester, permission: LocationPermission.whileInUse, pings: const []);

      expect(find.text('該日無軌跡資料'), findsOneWidget);
      expect(find.byType(FlutterMap), findsNothing);
    });

    testWidgets('permission-denied renders the primer and no map', (tester) async {
      await _pump(tester, permission: LocationPermission.denied, pings: const []);

      expect(find.text('尚未授權定位'), findsOneWidget);
      expect(find.text('前往系統設定'), findsOneWidget);
      expect(find.byType(FlutterMap), findsNothing);
    });

    testWidgets('changing the date dropdown triggers a refetch', (tester) async {
      final repo = _StubRepository(const []);
      await _pump(
        tester,
        permission: LocationPermission.whileInUse,
        pings: const [],
        repoOut: repo,
      );
      expect(repo.callCount, 1);

      await tester.tap(find.byKey(const ValueKey('trajectoryDateDropdown')));
      await tester.pumpAndSettle();

      // Pick the second entry — yesterday.
      final dayBefore = DateTime.now().subtract(const Duration(days: 1));
      final label =
          '${dayBefore.month.toString().padLeft(2, '0')}/${dayBefore.day.toString().padLeft(2, '0')}';
      await tester.tap(find.text(label).last);
      await tester.pumpAndSettle();

      expect(repo.callCount, 2);
    });
  });
}
