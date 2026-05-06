import 'dart:io' show Platform;

/// Compile-time + platform-aware API base URL resolver.
///
/// Resolution order at request time (see also `storage/api_base_url.dart` for
/// the runtime override layer applied on top in debug builds):
///
/// 1. `--dart-define=API_BASE_URL=...` if non-empty.
/// 2. `http://10.0.2.2:9090` on Android (host loopback for the emulator).
/// 3. `http://localhost:9090` on iOS Simulator and everywhere else.
class Env {
  const Env._();

  /// Provisional bundle id / app id. Mirrored from
  /// `ios/Runner.xcodeproj` and `android/app/build.gradle.kts`.
  static const String appId = 'tw.ccmos.app.bandao';

  static const String _dartDefine =
      String.fromEnvironment('API_BASE_URL');

  /// Compile-time privacy policy URL (admin-web `/privacy`). Override at
  /// build with `--dart-define=PRIVACY_URL=https://bandao.example.com/privacy`.
  /// At runtime the dev menu can override via secure storage; production
  /// builds rely on this constant.
  static const String _privacyUrlDartDefine =
      String.fromEnvironment('PRIVACY_URL');

  /// Lookup function for the current OS. Indirected so tests can override
  /// without monkey-patching `dart:io`.
  static bool Function() _isAndroid = () => Platform.isAndroid;

  /// Resolve the compile-time / platform default base URL.
  static String compileTimeDefault() {
    if (_dartDefine.isNotEmpty) {
      return _dartDefine;
    }
    if (_isAndroid()) {
      return 'http://10.0.2.2:9090';
    }
    return 'http://localhost:9090';
  }

  /// Compile-time default for the public privacy policy URL. Returns the
  /// dart-define value when present, else the dev admin-web URL on the
  /// matching loopback per platform.
  static String privacyUrlCompileTimeDefault() {
    if (_privacyUrlDartDefine.isNotEmpty) {
      return _privacyUrlDartDefine;
    }
    if (_isAndroid()) {
      return 'http://10.0.2.2:3000/privacy';
    }
    return 'http://localhost:3000/privacy';
  }

  /// Test hook: swap the OS check with a fake. Restore via [resetForTest].
  // @visibleForTesting — guarded by convention; we don't depend on package:meta directly.
  static void debugSetIsAndroid(bool Function() check) {
    _isAndroid = check;
  }

  /// Test hook: revert the OS check to the real `Platform.isAndroid`.
  // @visibleForTesting — guarded by convention; we don't depend on package:meta directly.
  static void resetForTest() {
    _isAndroid = () => Platform.isAndroid;
  }
}
