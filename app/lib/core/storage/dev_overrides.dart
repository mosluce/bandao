import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'secure_storage.dart';

/// Debug-only API base URL override readers / writers. Wraps the underlying
/// secure-storage calls but short-circuits in release builds so the override
/// path adds zero runtime work in production.
///
/// Decision: we tried the conditional-import pattern from design.md, but
/// Dart's `dart.library.X` switch only distinguishes web vs mobile — not
/// debug vs release. Using a `kReleaseMode` early-return achieves the same
/// dead-code-elimination because `kReleaseMode` is a const, so the tree-shake
/// removes the entire branch in release builds.
class DevOverrides {
  DevOverrides(this._storage);

  final SecureStorage _storage;

  /// Read the saved override, or `null` when not set / in release builds.
  Future<String?> read() async {
    if (kReleaseMode) {
      return null;
    }
    return _storage.readApiBaseUrlOverride();
  }

  /// Persist a new override. No-op in release builds.
  Future<void> write(String url) async {
    if (kReleaseMode) {
      return;
    }
    await _storage.writeApiBaseUrlOverride(url);
  }

  /// Clear the override, reverting to the compile-time default. No-op in
  /// release builds.
  Future<void> clear() async {
    if (kReleaseMode) {
      return;
    }
    await _storage.clearApiBaseUrlOverride();
  }
}

final devOverridesProvider = Provider<DevOverrides>((ref) {
  final storage = ref.watch(secureStorageProvider);
  return DevOverrides(storage);
});
