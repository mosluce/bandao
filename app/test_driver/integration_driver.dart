import 'dart:io';

import 'package:integration_test/integration_test_driver_extended.dart';

/// Flutter integration-test driver that persists `binding.takeScreenshot()`
/// calls to disk on the host machine.
///
/// Output directory comes from the `SCREENSHOT_OUT_DIR` host environment
/// variable. The `take_screenshots.sh` wrapper exports it per device class
/// so PNGs land directly under `app/store_metadata/ios/screenshots/{class}/`.
///
/// Why an env var, not `--dart-define`: `flutter drive --dart-define=...`
/// only injects compile-time defines into the device-side test process.
/// The driver runs on the host as a separate Dart program and never sees
/// those defines. `Platform.environment` does see the host env, which the
/// wrapper script exports before invoking flutter drive.
///
/// Run via:
///   SCREENSHOT_OUT_DIR=/abs/path flutter drive \
///     --driver=test_driver/integration_driver.dart \
///     --target=integration_test/screenshot_test.dart \
///     ...
Future<void> main() async {
  await integrationDriver(
    onScreenshot: (
      String name,
      List<int> bytes, [
      Map<String, Object?>? args,
    ]) async {
      final outDir =
          Platform.environment['SCREENSHOT_OUT_DIR'] ?? 'screenshots';
      final file = await File('$outDir/$name.png').create(recursive: true);
      await file.writeAsBytes(bytes);
      stdout.writeln('  → screenshot saved: $outDir/$name.png');
      return true;
    },
  );
}
