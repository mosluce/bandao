import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'secure_storage.dart';

/// Per-device API base URL override readers / writers. Thin wrapper over the
/// secure-storage calls.
///
/// This is a first-class feature in ALL build modes: the repo is public, so
/// self-hosters can point the shipped app at their own `api/` deployment via
/// the server-configuration screen. Validation of what may be stored (release
/// requires https) lives in `api_base_url.dart`'s `validateBaseUrlOverride`.
class ServerUrlOverride {
  ServerUrlOverride(this._storage);

  final SecureStorage _storage;

  /// Read the saved override, or `null` when not set.
  Future<String?> read() => _storage.readApiBaseUrlOverride();

  /// Persist a new override. Callers MUST validate first via
  /// `validateBaseUrlOverride`.
  Future<void> write(String url) => _storage.writeApiBaseUrlOverride(url);

  /// Clear the override, reverting to the compile-time default.
  Future<void> clear() => _storage.clearApiBaseUrlOverride();
}

final serverUrlOverrideProvider = Provider<ServerUrlOverride>((ref) {
  final storage = ref.watch(secureStorageProvider);
  return ServerUrlOverride(storage);
});
