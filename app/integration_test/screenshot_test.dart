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
import 'package:bandao_app/features/auth/state/auth_provider.dart';
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

    // Splash → either /login (cold start) or /home (token still valid
    // from a prior run). The org_code TextField is disabled until
    // LoginScreen's async `_loadLastOrgCode` resolves; pump-and-settle
    // once for splash, then again for the storage read.
    await tester.pumpAndSettle(const Duration(seconds: 3));
    await tester.pump(const Duration(milliseconds: 1500));
    await tester.pumpAndSettle();

    // If a previous run left a session token in secure_storage, the
    // router will land us on /home instead of /login. Log out
    // programmatically to bring us back to /login so we can capture the
    // login screen first.
    if (find.byKey(const Key('login.org_code')).evaluate().isEmpty) {
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      await container.read(authProvider.notifier).logout();
      // Auth listener fires → router redirects → splash settles → /login.
      await tester.pumpAndSettle(const Duration(seconds: 2));
      await tester.pump(const Duration(seconds: 1));
      await tester.pumpAndSettle();
    }

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
          'Could not reach /login even after a programmatic logout. The '
          'app may be stuck in a non-auth state — wipe the simulator '
          'completely and retry.',
    );
    await binding.takeScreenshot('01_login');

    // ─── login flow → /home ───────────────────────────────────────
    // integration-test text input on iOS simulator silently drops
    // enterText calls when the target field isn't already focused.
    // Tap → settle → enterText → settle reliably populates the field.
    Future<void> fillField(String keyName, String value) async {
      final finder = find.byKey(Key(keyName));
      await tester.tap(finder);
      await tester.pumpAndSettle();
      await tester.enterText(finder, value);
      await tester.pumpAndSettle();
    }

    await fillField('login.org_code', _orgCode);
    await fillField('login.username', _username);
    await fillField('login.password', _password);

    // The password field has `onSubmitted: _submit` wired up, so sending
    // the "done" keyboard action triggers login AND dismisses the
    // keyboard in one shot. Tapping login.submit explicitly afterwards
    // would race with the post-login navigation — by the time we'd find
    // the button, /home has replaced /login and login.submit no longer
    // exists, throwing "Found 0 widgets with key login.submit".
    await tester.testTextInput.receiveAction(TextInputAction.done);

    // Backend roundtrip + AuthProvider state update + go_router redirect.
    // The Bandao app has periodic timers (queue processor / location
    // pings) that keep pumpAndSettle from quiescing quickly. Mix
    // pumpAndSettle with explicit pump() to give the network call real
    // time to complete.
    await tester.pumpAndSettle(const Duration(milliseconds: 500));
    await tester.pump(const Duration(seconds: 4));
    await tester.pumpAndSettle(const Duration(milliseconds: 500));

    // ─── 02_home ──────────────────────────────────────────────────
    await binding.takeScreenshot('02_home');

    // ─── 03_history ───────────────────────────────────────────────
    final ctx = tester.element(find.byType(Scaffold).first);
    GoRouter.of(ctx).go(AppRoutes.history);
    await tester.pumpAndSettle(const Duration(milliseconds: 500));
    await tester.pump(const Duration(seconds: 2));
    await tester.pumpAndSettle(const Duration(milliseconds: 500));
    await binding.takeScreenshot('03_history');
  });
}
