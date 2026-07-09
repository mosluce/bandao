import 'package:flutter/foundation.dart' show kReleaseMode;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../env/env.dart';
import 'server_url_override.dart';

/// Why a candidate base-URL override was rejected. `null` (from
/// [validateBaseUrlOverride]) means it is acceptable.
enum BaseUrlOverrideError {
  /// Not a parseable URL with a scheme and a host.
  malformed,

  /// Release builds require `https`; this value used another scheme.
  insecureScheme,
}

/// Validates a candidate API base-URL override before it is persisted.
///
/// Release builds require an `https` URL with a host — this both protects the
/// user's credentials on the public internet and means the app needs no iOS
/// ATS / Android cleartext exception. Debug builds stay loose (any scheme +
/// host) so `http://localhost:9090` and LAN IPs work for local development.
///
/// [release] defaults to [kReleaseMode]; tests inject it to exercise the
/// release path (which `flutter test` runs in debug otherwise).
BaseUrlOverrideError? validateBaseUrlOverride(String url, {bool? release}) {
  final parsed = Uri.tryParse(url);
  if (parsed == null || !parsed.hasScheme || !parsed.hasAuthority) {
    return BaseUrlOverrideError.malformed;
  }
  if ((release ?? kReleaseMode) && parsed.scheme != 'https') {
    return BaseUrlOverrideError.insecureScheme;
  }
  return null;
}

/// Resolves the effective API base URL by layering the per-device override on
/// top of the platform-aware compile-time default.
///
/// Returns the override when present (self-hosted server, all build modes),
/// otherwise [Env.compileTimeDefault]. What may be stored as an override is
/// gated by [validateBaseUrlOverride] at write time.
class ApiBaseUrlResolver {
  ApiBaseUrlResolver(this._overrides);

  final ServerUrlOverride _overrides;

  Future<String> effectiveBaseUrl() async {
    final override = await _overrides.read();
    if (override != null && override.isNotEmpty) {
      return override;
    }
    return Env.compileTimeDefault();
  }
}

final apiBaseUrlResolverProvider = Provider<ApiBaseUrlResolver>((ref) {
  final overrides = ref.watch(serverUrlOverrideProvider);
  return ApiBaseUrlResolver(overrides);
});

/// One-shot async fetch of the effective base URL. Riverpod's caching means
/// each call after the first within the same provider scope returns the same
/// value until the resolver is invalidated (e.g. on server-config save).
final effectiveBaseUrlProvider = FutureProvider<String>((ref) async {
  final resolver = ref.watch(apiBaseUrlResolverProvider);
  return resolver.effectiveBaseUrl();
});
