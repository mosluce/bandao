import 'package:dio/dio.dart';
import 'package:logger/logger.dart';
import 'package:workmanager/workmanager.dart';

import '../../../core/api/api_error.dart';
import '../../../core/api/auth_interceptor.dart';
import '../../../core/api/error_interceptor.dart';
import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/submit_checkin_event.dart';
import '../../../core/env/env.dart';
import '../../../core/storage/secure_storage.dart';
import 'checkin_queue_db.dart';
import 'checkin_repository.dart';

const String kQueueDrainTaskName = 'tw.ccmos.app.bandao.queue-drain';

/// Total budget for one background invocation. iOS gives ~25s for a
/// `BGProcessingTask`; we keep some margin.
const Duration _backgroundBudget = Duration(seconds: 22);

/// Top-level entry-point invoked from a background isolate. Workmanager
/// requires it to be a top-level (or static) function and annotated so the
/// AOT compiler keeps it.
@pragma('vm:entry-point')
void backgroundCallbackDispatcher() {
  Workmanager().executeTask((task, inputData) async {
    try {
      await runBackgroundDrain();
      return true;
    } catch (e) {
      // Returning false signals the OS we'd like a retry; for our case any
      // remaining row will be picked up by the next foreground tick anyway,
      // so we don't insist on a retry — return true.
      return true;
    }
  });
}

/// Drains the queue without Riverpod context. Used by the background
/// callback. Foreground uses `QueueProcessor` instead, which shares the same
/// protocol but reads dependencies through Riverpod.
Future<void> runBackgroundDrain() async {
  final log = Logger();
  final db = CheckinQueueDb();
  try {
    final storage = SecureStorage();
    final token = await storage.readToken();
    if (token == null || token.isEmpty) {
      log.i('background drain: no token, skipping');
      return;
    }

    final overrideUrl = await storage.readApiBaseUrlOverride();
    final baseUrl =
        (overrideUrl != null && overrideUrl.isNotEmpty)
            ? overrideUrl
            : Env.compileTimeDefault();

    final dio = Dio(
      BaseOptions(
        baseUrl: baseUrl,
        connectTimeout: const Duration(seconds: 10),
        receiveTimeout: const Duration(seconds: 15),
        sendTimeout: const Duration(seconds: 15),
        contentType: 'application/json',
        responseType: ResponseType.json,
      ),
    );
    dio.interceptors.add(AuthInterceptor(storage));
    dio.interceptors.add(const ErrorInterceptor());
    final repo = CheckinRepository(dio);

    final deadline = DateTime.now().add(_backgroundBudget);

    while (DateTime.now().isBefore(deadline)) {
      final next = await db.pickOldestPending();
      if (next == null) break;

      // Honor backoff (last_attempt_at + nextDelay).
      if (next.lastAttemptAt != null && next.attempts > 0) {
        final lastAt = DateTime.tryParse(next.lastAttemptAt!);
        if (lastAt != null) {
          final elapsed = DateTime.now().difference(lastAt);
          final required = _backoff(next.attempts);
          if (elapsed < required) break;
        }
      }

      await db.markSending(next.id);
      try {
        await repo.submit(_toRequest(next));
        await db.deleteRow(next.id);
        continue;
      } on ApiException catch (e) {
        switch (e.code) {
          case 'INVALID_TRANSITION':
          case 'OUT_OF_ORDER':
          case 'TRANSFER_DISABLED':
          case 'NEEDS_PASSWORD_CHANGE':
            await db.markFailed(
              next.id,
              errorCode: e.code,
              errorMessage: e.message,
            );
            continue;
        }
        if (e.status == 401 || e.code == ApiErrorCode.unauthorized) {
          await db.markFailed(
            next.id,
            errorCode: e.code,
            errorMessage: e.message,
          );
          // Background can't navigate; foreground will pick this up via the
          // failed-row visibility and the next /me call.
          break;
        }
        // 5xx / network — return to pending and stop. Next tick (foreground
        // or future workmanager) will retry per the backoff.
        await db.markPending(
          next.id,
          lastErrorCode: e.code,
          lastErrorMessage: e.message,
        );
        break;
      } catch (e) {
        await db.markPending(
          next.id,
          lastErrorCode: 'UNKNOWN',
          lastErrorMessage: e.toString(),
        );
        break;
      }
    }
  } finally {
    await db.close();
  }
}

Duration _backoff(int attempts) {
  const sched = [1, 2, 4, 8, 16, 30];
  final idx = (attempts - 1).clamp(0, sched.length - 1);
  return Duration(seconds: sched[idx]);
}

SubmitCheckinEventRequest _toRequest(QueueRow row) {
  return SubmitCheckinEventRequest(
    eventType: CheckinEventType.fromJson(row.eventType),
    lat: row.lat,
    lng: row.lng,
    accuracy: row.accuracy,
    manualLabel: row.manualLabel,
    occurredAtClient: row.occurredAtClient,
  );
}

/// Initialize Workmanager. Call once from `main()` after
/// `WidgetsFlutterBinding.ensureInitialized()`.
Future<void> initBackgroundSync() async {
  await Workmanager().initialize(backgroundCallbackDispatcher);
}

/// Schedule a one-off background drain. Called from the enqueue path and on
/// app start. On Android this becomes a `OneTimeWorkRequest` constrained to
/// connected networks. On iOS the unique-name routes through to the
/// BGProcessingTask scheduler.
Future<void> requestBackgroundDrain() async {
  try {
    await Workmanager().registerOneOffTask(
      kQueueDrainTaskName,
      kQueueDrainTaskName,
      existingWorkPolicy: ExistingWorkPolicy.keep,
      constraints: Constraints(networkType: NetworkType.connected),
    );
  } catch (e) {
    // Workmanager registration is best-effort — failures here just mean the
    // OS won't wake us in background; foreground tick still drains.
  }
}
