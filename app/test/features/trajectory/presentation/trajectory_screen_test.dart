import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';

import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/checkin/data/checkin_repository.dart';
import 'package:bandao_app/features/checkin/data/geolocation_service.dart';
import 'package:bandao_app/features/trajectory/data/my_locations_repository.dart';
import 'package:bandao_app/features/trajectory/data/trajectory_merge.dart';
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

/// The day's events feed both the markers and — via the merge contract — the
/// line's endpoints, so tests control them explicitly.
class _StubCheckinRepo implements CheckinRepository {
  _StubCheckinRepo([this._events = const <CheckinEventDto>[]]);

  final List<CheckinEventDto> _events;

  @override
  Future<List<CheckinEventDto>> events({
    String? before,
    int limit = 50,
    DateTime? from,
    DateTime? to,
  }) async =>
      _events;

  @override
  dynamic noSuchMethod(Invocation invocation) =>
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
  List<CheckinEventDto> events = const <CheckinEventDto>[],
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
        checkinRepositoryProvider
            .overrideWith((ref) async => _StubCheckinRepo(events)),
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

CheckinEventDto _event(String id, String iso, double lat, double lng) =>
    CheckinEventDto(
      id: id,
      appUserId: 'u1',
      eventType: CheckinEventType.clockIn,
      occurredAtClient: iso,
      occurredAtServer: iso,
      source: EventSource.app,
      initiatedByKind: EventInitiatorKind.appUser,
      initiatedById: 'u1',
      location: EventLocation(coordinates: GeoPoint(lat: lat, lng: lng)),
      hasSkewWarning: false,
    );

MergedPoint _pt(String id, String iso, double lat, double lng) => MergedPoint(
      lat: lat,
      lng: lng,
      occurredAtClient: iso,
      originRank: originEvent,
      id: id,
    );

void main() {
  group('cameraPointsFor', () {
    // Regression: an event contributes the same LatLng twice — once as a
    // merged-series vertex, once as a marker. Without dedup a clock-in with
    // no pings yielded two entries at one place, `length > 1` chose the
    // fitBounds path, and a zero-area bounds made flutter_map compute an
    // infinite zoom that TileLayer throws on. Every shift passes through
    // that state between clocking in and the first ping.
    test('a clock-in with no pings collapses to ONE distinct point', () {
      final e = _event('in', '2026-07-31T00:49:08Z', 22.6116, 120.3006);
      final series = [_pt('in', '2026-07-31T00:49:08Z', 22.6116, 120.3006)];

      expect(cameraPointsFor(series, [e]), hasLength(1));
    });

    test('distinct coordinates are all kept', () {
      final e = _event('in', '2026-07-31T00:49:08Z', 22.6116, 120.3006);
      final series = [
        _pt('in', '2026-07-31T00:49:08Z', 22.6116, 120.3006),
        _pt('p1', '2026-07-31T01:00:00Z', 22.7000, 120.4000),
      ];

      expect(cameraPointsFor(series, [e]), hasLength(2));
    });

    test('an excluded event still contributes its marker to the camera', () {
      // admin_force is kept out of the line but must stay in view.
      final forced = _event('f', '2026-07-31T09:00:00Z', 24.0, 120.0);
      final series = [_pt('in', '2026-07-31T00:49:08Z', 22.6116, 120.3006)];

      expect(cameraPointsFor(series, [forced]), hasLength(2));
    });

    test('no data yields no camera points', () {
      expect(cameraPointsFor(const [], const []), isEmpty);
    });
  });

  group('TrajectoryScreen', () {
    // The "with-data" path mounts FlutterMap, which fetches network tiles
    // and TestWidgetsFlutterBinding always returns 400 → uncaught exception.
    // The data branch is covered indirectly:
    //   - TrajectoryController test verifies pings → stats computation
    //   - §11 smoke verifies the map+stats render on a real device.
    // The widget tests below stick to branches that don't mount the map.

    testWidgets('empty-day path renders the empty text and no map',
        (tester) async {
      await _pump(
        tester,
        permission: LocationPermission.whileInUse,
        pings: const [],
      );

      expect(find.text('該日無軌跡資料'), findsOneWidget);
      expect(find.byType(FlutterMap), findsNothing);
    });

    testWidgets('permission-denied renders the primer and no map',
        (tester) async {
      await _pump(
        tester,
        permission: LocationPermission.denied,
        pings: const [],
      );

      expect(find.text('尚未授權定位'), findsOneWidget);
      expect(find.text('前往系統設定'), findsOneWidget);
      expect(find.byType(FlutterMap), findsNothing);
    });

    // The `>= 1 merged point` branches mount FlutterMap, whose tile fetches
    // always 400 under TestWidgetsFlutterBinding (see the note above), so the
    // render thresholds are asserted on the controller's `mergedSeries` in
    // `trajectory_controller_test.dart` instead — that is the value the
    // widget's branch reads. The drawn geometry itself is covered by
    // admin-web's `trajectory.test.ts`, which asserts real polyline vertices
    // against the same merge contract, plus the §6.4 device smoke.

    testWidgets('changing the date dropdown triggers a refetch',
        (tester) async {
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
