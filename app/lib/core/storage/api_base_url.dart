import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../env/env.dart';
import 'dev_overrides.dart';

/// Resolves the effective API base URL by layering the debug-only override
/// on top of the platform-aware compile-time default.
///
/// Returns the override when present (debug only — `DevOverrides.read()`
/// short-circuits to `null` in release), otherwise [Env.compileTimeDefault].
class ApiBaseUrlResolver {
  ApiBaseUrlResolver(this._overrides);

  final DevOverrides _overrides;

  Future<String> effectiveBaseUrl() async {
    final override = await _overrides.read();
    if (override != null && override.isNotEmpty) {
      return override;
    }
    return Env.compileTimeDefault();
  }
}

final apiBaseUrlResolverProvider = Provider<ApiBaseUrlResolver>((ref) {
  final overrides = ref.watch(devOverridesProvider);
  return ApiBaseUrlResolver(overrides);
});

/// One-shot async fetch of the effective base URL. Riverpod's caching means
/// each call after the first within the same provider scope returns the same
/// value until the resolver is invalidated (e.g. on dev menu save).
final effectiveBaseUrlProvider = FutureProvider<String>((ref) async {
  final resolver = ref.watch(apiBaseUrlResolverProvider);
  return resolver.effectiveBaseUrl();
});
