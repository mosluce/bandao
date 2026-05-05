import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../env/env.dart';
import 'secure_storage.dart';

/// Resolves the effective privacy policy URL by layering the debug-only
/// override on top of the compile-time default. Mirrors `ApiBaseUrlResolver`.
class PrivacyUrlResolver {
  PrivacyUrlResolver(this._storage);

  final SecureStorage _storage;

  Future<String> effectivePrivacyUrl() async {
    final override = await _storage.readPrivacyUrlOverride();
    if (override != null && override.isNotEmpty) {
      return override;
    }
    return Env.privacyUrlCompileTimeDefault();
  }
}

final privacyUrlResolverProvider = Provider<PrivacyUrlResolver>((ref) {
  final storage = ref.watch(secureStorageProvider);
  return PrivacyUrlResolver(storage);
});

/// One-shot async fetch — cached per provider scope.
final effectivePrivacyUrlProvider = FutureProvider<String>((ref) async {
  final resolver = ref.watch(privacyUrlResolverProvider);
  return resolver.effectivePrivacyUrl();
});
