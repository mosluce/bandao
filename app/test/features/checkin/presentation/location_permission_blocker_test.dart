import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';

import 'package:argus_app/core/api/models/checkin_event.dart';
import 'package:argus_app/features/checkin/data/geolocation_service.dart';
import 'package:argus_app/features/checkin/presentation/location_permission_blocker.dart';
import 'package:argus_app/features/checkin/state/location_permission_provider.dart';
import 'package:argus_app/l10n/app_localizations.dart';

void main() {
  group('LocationPermissionBlocker', () {
    testWidgets('hidden when permission is granted', (tester) async {
      await _pump(tester, permission: LocationPermission.whileInUse);
      expect(find.text('需要定位權限才能打卡'), findsNothing);
      expect(find.text('開啟設定'), findsNothing);
    });

    testWidgets('hidden when permission is denied (never-asked)',
        (tester) async {
      // `denied` covers the iOS first-install "notDetermined" state — the
      // blocker stays hidden so the user can tap a button to trigger the
      // system permission dialog.
      await _pump(tester, permission: LocationPermission.denied);
      expect(find.text('需要定位權限才能打卡'), findsNothing);
    });

    testWidgets('visible when permission is deniedForever', (tester) async {
      await _pump(tester, permission: LocationPermission.deniedForever);
      expect(find.text('需要定位權限才能打卡'), findsOneWidget);
      expect(find.text('開啟設定'), findsOneWidget);
    });

    testWidgets('Open settings button calls openSettings on the service',
        (tester) async {
      final svc = _RecordingGeolocator(LocationPermission.deniedForever);
      await _pump(
        tester,
        permission: LocationPermission.deniedForever,
        service: svc,
      );
      await tester.tap(find.text('開啟設定'));
      await tester.pump();
      expect(svc.openSettingsCalls, 1);
    });
  });
}

Future<void> _pump(
  WidgetTester tester, {
  required LocationPermission permission,
  GeolocationService? service,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        if (service != null)
          geolocationServiceProvider.overrideWithValue(service),
        locationPermissionProvider.overrideWith(
          () => _FixedPermissionNotifier(permission),
        ),
      ],
      child: MaterialApp(
        home: const Scaffold(body: LocationPermissionBlocker()),
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
  await tester.pumpAndSettle();
}

class _FixedPermissionNotifier extends LocationPermissionNotifier {
  _FixedPermissionNotifier(this.fixed);
  final LocationPermission fixed;

  @override
  Future<LocationPermission> build() async => fixed;
}

class _RecordingGeolocator implements GeolocationService {
  _RecordingGeolocator(this._permission);
  final LocationPermission _permission;
  int openSettingsCalls = 0;

  @override
  Future<LocationPermission> currentPermission() async => _permission;

  @override
  Future<LocationPermission> requestPermission() async => _permission;

  @override
  Future<({GeoPoint point, double? accuracyMeters})?> capture() async => null;

  @override
  Future<bool> openSettings() async {
    openSettingsCalls++;
    return true;
  }
}
