import 'package:firebase_crashlytics/firebase_crashlytics.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Sink for non-fatal failures the app recovers from but still needs to know
/// about.
///
/// Exists as a seam rather than calling `FirebaseCrashlytics.instance`
/// directly so that (a) unit tests can assert a failure was reported without
/// initialising Firebase, and (b) recoverable failures are reported through
/// one path instead of each caller deciding.
abstract class ErrorReporter {
  const ErrorReporter();

  /// Record a handled failure. Must never throw — a reporting problem must
  /// not become the caller's problem.
  void recordNonFatal(
    Object error,
    StackTrace stackTrace, {
    String? reason,
    Map<String, Object?> context = const <String, Object?>{},
  });
}

/// Production reporter. Swallows its own failures: telemetry is never worth
/// crashing a recovered code path for.
class CrashlyticsErrorReporter extends ErrorReporter {
  const CrashlyticsErrorReporter();

  @override
  void recordNonFatal(
    Object error,
    StackTrace stackTrace, {
    String? reason,
    Map<String, Object?> context = const <String, Object?>{},
  }) {
    try {
      final crashlytics = FirebaseCrashlytics.instance;
      for (final entry in context.entries) {
        crashlytics.setCustomKey(entry.key, entry.value.toString());
      }
      crashlytics.recordError(
        error,
        stackTrace,
        reason: reason,
        fatal: false,
      );
    } catch (_) {
      // Firebase not initialised, or the plugin itself failed. Nothing useful
      // to do — the caller has already handled the original failure.
    }
  }
}

/// Drops everything. The default for objects constructed outside the provider
/// graph (unit tests), so a test never needs Firebase to exist.
class NoopErrorReporter extends ErrorReporter {
  const NoopErrorReporter();

  @override
  void recordNonFatal(
    Object error,
    StackTrace stackTrace, {
    String? reason,
    Map<String, Object?> context = const <String, Object?>{},
  }) {}
}

final errorReporterProvider = Provider<ErrorReporter>((ref) {
  return const CrashlyticsErrorReporter();
});
