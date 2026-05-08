import 'dart:io';

import 'package:integration_test/integration_test_driver_extended.dart';

/// Flutter integration-test driver that persists `binding.takeScreenshot()`
/// calls to disk on the host machine.
///
/// Output directory comes from `--dart-define=SCREENSHOT_OUT_DIR=...`. The
/// `take_screenshots.sh` wrapper sets it per device class so PNGs land
/// directly under `app/store_metadata/ios/screenshots/{iphone_6.7,...}`.
///
/// Run via:
///   flutter drive \
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
      const outDir = String.fromEnvironment(
        'SCREENSHOT_OUT_DIR',
        defaultValue: 'screenshots',
      );
      final file = await File('$outDir/$name.png').create(recursive: true);
      await file.writeAsBytes(bytes);
      stdout.writeln('  → screenshot saved: $outDir/$name.png');
      return true;
    },
  );
}
