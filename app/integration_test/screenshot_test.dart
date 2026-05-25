// Automated store-metadata screenshot pipeline.
//
// Run via the wrapper at `app/scripts/take_screenshots.sh` (iOS) or
// `app/scripts/take_android_screenshots.sh` (Android). The wrapper boots
// a simulator/emulator for the device class, runs `flutter drive`
// against this test, and persists each captured PNG to
// `app/store_metadata/{ios,android}/...`.
//
// Capture mechanism differs by platform:
//   - iOS: `binding.takeScreenshot(name)` writes via the integration_test
//     callback bus to the host driver in `test_driver/integration_driver.dart`.
//   - Android: integration_test 4.x's `takeScreenshot` hangs indefinitely
//     on Android emulators after `convertFlutterSurfaceToImage`. We dodge
//     by printing a `SHOOT:<name>` marker to stdout and pausing briefly;
//     the host script tails stdout, runs `adb exec-out screencap -p`, and
//     writes the PNG itself. The Dart side never touches the Flutter
//     surface — bulletproof against renderer state issues.
//
// Credentials are passed via `--dart-define` so they never enter the repo.

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

    // The Bandao app has 1 Hz periodic timers (queue_processor,
    // location_tracking_service, location_ping_processor). Those keep
    // `pumpAndSettle` from ever quiescing — it hangs to its 10-minute
    // internal timeout. Use explicit pump loops with a fixed wall-clock
    // budget instead.
    Future<void> pumpFor(Duration total) async {
      const frame = Duration(milliseconds: 100);
      var elapsed = Duration.zero;
      while (elapsed < total) {
        await tester.pump(frame);
        elapsed += frame;
      }
    }

    // Platform-aware screenshot capture. See the file header for why
    // Android takes a different path.
    Future<void> capture(String name) async {
      if (Platform.isAndroid) {
        // ignore: avoid_print
        print('SHOOT:$name');
        // Give the host script time to run adb exec-out screencap before
        // the next interaction perturbs the frame.
        await Future.delayed(const Duration(seconds: 2));
      } else {
        await binding.takeScreenshot(name);
      }
    }

    // Splash → /login (cold) or /home (token still valid). The
    // org_code TextField is disabled until LoginScreen's async
    // `_loadLastOrgCode` resolves; pump generously to cover splash +
    // the secure_storage read.
    await pumpFor(const Duration(seconds: 5));

    // If a stale session token sent us to /home, log out so we can
    // capture the login screen first.
    if (find.byKey(const Key('login.org_code')).evaluate().isEmpty) {
      final container = ProviderScope.containerOf(
        tester.element(find.byType(MaterialApp)),
      );
      await container.read(authProvider.notifier).logout();
      await pumpFor(const Duration(seconds: 3));
    }

    // iOS-only: swap the live surface for an image-backed render so
    // `binding.takeScreenshot()` can sample pixels.
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
    await capture('01_login');

    // ─── login flow → /home ───────────────────────────────────────
    // integration-test text input on iOS simulator silently drops
    // enterText calls when the target field isn't already focused.
    // Tap → pump → enterText → pump reliably populates the field.
    Future<void> fillField(String keyName, String value) async {
      final finder = find.byKey(Key(keyName));
      await tester.tap(finder);
      await pumpFor(const Duration(milliseconds: 500));
      await tester.enterText(finder, value);
      await pumpFor(const Duration(milliseconds: 500));
    }

    await fillField('login.org_code', _orgCode);
    await fillField('login.username', _username);
    await fillField('login.password', _password);

    // The password field has `onSubmitted: _submit` wired up, so the
    // "done" keyboard action triggers login AND dismisses the keyboard
    // in one shot. Tapping login.submit explicitly would race with the
    // post-login navigation.
    await tester.testTextInput.receiveAction(TextInputAction.done);

    // Backend roundtrip + AuthProvider state update + go_router redirect.
    await pumpFor(const Duration(seconds: 6));

    // ─── 02_home ──────────────────────────────────────────────────
    await capture('02_home');

    // ─── 03_history ───────────────────────────────────────────────
    final ctx = tester.element(find.byType(Scaffold).first);
    GoRouter.of(ctx).go(AppRoutes.history);
    await pumpFor(const Duration(seconds: 3));
    await capture('03_history');

    // ─── 04_trajectory ────────────────────────────────────────────
    // The "我的工作日記" tab — the AppUser-facing surface that
    // justifies UIBackgroundModes:location to App Review 2.5.4.
    // Whether the polyline renders depends on the demo Org having
    // pings persisted for today; off-data the screen shows the
    // empty state. See DEPLOY.md "App Review submission checklist".
    GoRouter.of(ctx).go(AppRoutes.trajectory);
    // The trajectory screen fires off the GET /app/checkin/me/locations
    // on first build; give the network round-trip + flutter_map's
    // initial tile fetch room to settle before sampling pixels.
    await pumpFor(const Duration(seconds: 8));
    await capture('04_trajectory');
  });
}
