// Automated store-metadata screenshot pipeline.
//
// Run via the wrapper at `app/scripts/take_screenshots.sh`. The wrapper
// boots a simulator for the device class, runs `flutter drive` against
// this test, and the driver in `test_driver/integration_driver.dart`
// writes each captured PNG to `app/store_metadata/ios/screenshots/<class>/`
// (or the equivalent android directory).
//
// Credentials are passed via `--dart-define` flags so they never enter
// the repo. See script for the list of required defines.

import 'dart:io';

import 'package:bandao_app/app/bandao_app.dart';
import 'package:bandao_app/app/router.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:integration_test/integration_test.dart';

const String _orgCode = String.fromEnvironment(
  'TEST_ORG_CODE',
  defaultValue: '',
);
const String _username = String.fromEnvironment(
  'TEST_USERNAME',
  defaultValue: '',
);
const String _password = String.fromEnvironment(
  'TEST_PASSWORD',
  defaultValue: '',
);

void main() {
  final binding = IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('Capture store-metadata screenshots', (tester) async {
    if (_orgCode.isEmpty || _username.isEmpty || _password.isEmpty) {
      fail(
        'Missing credentials. Pass --dart-define=TEST_ORG_CODE=... '
        '--dart-define=TEST_USERNAME=... --dart-define=TEST_PASSWORD=... '
        '— easiest via app/scripts/take_screenshots.sh.',
      );
    }

    runApp(const ProviderScope(child: BandaoApp()));

    // Splash → /login redirect for an unauthenticated cold start.
    await tester.pumpAndSettle(const Duration(seconds: 5));

    // iOS requires switching the surface to an image-backed render before
    // takeScreenshot() can sample pixels. No-op on Android.
    if (Platform.isIOS) {
      await binding.convertFlutterSurfaceToImage();
    }

    // ─── 01_login ─────────────────────────────────────────────────
    expect(
      find.byKey(const Key('login.org_code')),
      findsOneWidget,
      reason:
          'Cold-start should land on /login. If this fails, the simulator '
          'still has a session cookie/token from a prior run — wipe the '
          'app or use a fresh simulator.',
    );
    await binding.takeScreenshot('01_login');

    // ─── login flow → /home ───────────────────────────────────────
    await tester.enterText(
      find.byKey(const Key('login.org_code')),
      _orgCode,
    );
    await tester.enterText(
      find.byKey(const Key('login.username')),
      _username,
    );
    await tester.enterText(
      find.byKey(const Key('login.password')),
      _password,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('login.submit')));
    // Backend roundtrip + AuthProvider state update + go_router redirect.
    // 8s is generous; cut down if it stays consistently fast.
    await tester.pumpAndSettle(const Duration(seconds: 8));

    // ─── 02_home ──────────────────────────────────────────────────
    await binding.takeScreenshot('02_home');

    // ─── 03_history ───────────────────────────────────────────────
    final ctx = tester.element(find.byType(Scaffold).first);
    GoRouter.of(ctx).go(AppRoutes.history);
    await tester.pumpAndSettle(const Duration(seconds: 3));
    await binding.takeScreenshot('03_history');
  });
}
