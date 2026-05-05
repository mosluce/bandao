import 'package:drift/drift.dart' show Value;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/api_error.dart';
import 'package:argus_app/core/api/models/checkin_event.dart';
import 'package:argus_app/core/api/models/checkin_status.dart';
import 'package:argus_app/core/api/models/submit_checkin_event.dart';
import 'package:argus_app/features/checkin/data/checkin_queue_db.dart';
import 'package:argus_app/features/checkin/data/checkin_repository.dart';
import 'package:argus_app/features/checkin/data/queue_processor.dart';

void main() {
  group('QueueProcessor.nextDelay', () {
    test('attempts 1..6 walks the schedule', () {
      expect(QueueProcessor.nextDelay(1), const Duration(seconds: 1));
      expect(QueueProcessor.nextDelay(2), const Duration(seconds: 2));
      expect(QueueProcessor.nextDelay(3), const Duration(seconds: 4));
      expect(QueueProcessor.nextDelay(4), const Duration(seconds: 8));
      expect(QueueProcessor.nextDelay(5), const Duration(seconds: 16));
      expect(QueueProcessor.nextDelay(6), const Duration(seconds: 30));
    });

    test('caps at 30s', () {
      expect(QueueProcessor.nextDelay(7), const Duration(seconds: 30));
      expect(QueueProcessor.nextDelay(99), const Duration(seconds: 30));
    });
  });

  group('QueueProcessor.tick', () {
    late CheckinQueueDb db;
    late _FakeRepo repo;
    bool authExpiredCalled = false;
    final syncedEvents = <CheckinEventDto>[];
    final freshStatuses = <CheckinUserStatusDto>[];

    setUp(() {
      db = CheckinQueueDb.forTesting(NativeDatabase.memory());
      repo = _FakeRepo();
      authExpiredCalled = false;
      syncedEvents.clear();
      freshStatuses.clear();
      addTearDown(() async {
        await db.close();
      });
    });

    QueueProcessor build({bool online = true}) {
      return QueueProcessor(
        db: db,
        repo: () async => repo,
        isOnline: () => online,
        onAuthExpired: () async {
          authExpiredCalled = true;
        },
        onStatusFresh: freshStatuses.add,
        onEventSynced: syncedEvents.add,
      );
    }

    test('201 success deletes row, advances, and pushes fresh state', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      await _enqueue(db, 'transfer_out', '2026-05-04T10:00:00+08:00');

      repo.responses.add(_okResponse('e1'));
      repo.responses.add(_okResponse('e2'));

      await build().tick();
      final all = await db.allForUser('u1');
      expect(all, isEmpty);
      expect(repo.calls, 2);
      // onEventSynced callback fires with each response's event.
      expect(syncedEvents.map((e) => e.id), ['e1', 'e2']);
      // onStatusFresh callback fires with each response's status.
      expect(freshStatuses.length, 2);
    });

    test('INVALID_TRANSITION marks failed and advances', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      await _enqueue(db, 'clock_out', '2026-05-04T10:00:00+08:00');

      repo.responses.add(_throw(ApiException(
        status: 422,
        code: 'INVALID_TRANSITION',
        message: 'cannot clock_in from on_site',
      ),),);
      repo.responses.add(_okResponse('e2'));

      await build().tick();
      final all = await db.allForUser('u1');
      expect(all.length, 1);
      expect(all.first.status, 'failed');
      expect(all.first.lastErrorCode, 'INVALID_TRANSITION');
    });

    test('OUT_OF_ORDER marks failed', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      repo.responses.add(_throw(ApiException(
        status: 409,
        code: 'OUT_OF_ORDER',
        message: 'event before previous',
      ),),);
      await build().tick();
      final all = await db.allForUser('u1');
      expect(all.first.status, 'failed');
      expect(all.first.lastErrorCode, 'OUT_OF_ORDER');
    });

    test('TRANSFER_DISABLED marks failed', () async {
      await _enqueue(db, 'transfer_out', '2026-05-04T08:00:00+08:00');
      repo.responses.add(_throw(ApiException(
        status: 403,
        code: 'TRANSFER_DISABLED',
        message: 'transfer disabled',
      ),),);
      await build().tick();
      final all = await db.allForUser('u1');
      expect(all.first.status, 'failed');
      expect(all.first.lastErrorCode, 'TRANSFER_DISABLED');
    });

    test('500 returns row to pending and stops', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      await _enqueue(db, 'transfer_out', '2026-05-04T10:00:00+08:00');
      repo.responses.add(_throw(ApiException(
        status: 500,
        code: 'INTERNAL',
        message: 'oops',
      ),),);
      await build().tick();
      final all = await db.allForUser('u1');
      expect(all.length, 2);
      // First row back to pending, attempts incremented.
      final first = all.firstWhere((r) => r.eventType == 'clock_in');
      expect(first.status, 'pending');
      expect(first.attempts, 1);
      // Only one repo call — single in-flight, no advance after retryable.
      expect(repo.calls, 1);
    });

    test('network error returns row to pending', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      repo.responses.add(_throw(ApiException.network('boom')));
      await build().tick();
      final all = await db.allForUser('u1');
      expect(all.first.status, 'pending');
      expect(all.first.attempts, 1);
      expect(all.first.lastErrorCode, 'NETWORK_ERROR');
    });

    test('offline skip — does not mark sending or burn attempts', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      await build(online: false).tick();

      final all = await db.allForUser('u1');
      expect(all.first.status, 'pending');
      expect(all.first.attempts, 0);
      expect(repo.calls, 0);
    });

    test('401 marks failed and signals auth expired', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      repo.responses.add(_throw(ApiException.unauthorized()));
      await build().tick();
      final all = await db.allForUser('u1');
      expect(all.first.status, 'failed');
      expect(authExpiredCalled, isTrue);
    });

    test('single in-flight: skip when a sending row exists', () async {
      // Pre-mark a row as sending to simulate concurrent state.
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');
      final r = await db.pickOldestPending();
      await db.markSending(r!.id);

      await _enqueue(db, 'clock_out', '2026-05-04T17:00:00+08:00');
      await build().tick();
      // Tick saw the sending row and bailed out — no repo calls.
      expect(repo.calls, 0);
    });

    test('backoff window: a row recently attempted is not retried', () async {
      await _enqueue(db, 'clock_in', '2026-05-04T08:00:00+08:00');

      repo.responses.add(_throw(ApiException(
        status: 500,
        code: 'INTERNAL',
        message: 'oops',
      ),),);
      await build().tick();
      // After failure, attempts=1, last_attempt_at=now. Next tick should skip.
      repo.responses.add(_okResponse('e1'));
      await build().tick();
      expect(repo.calls, 1); // didn't make a second call within backoff.
    });
  });
}

Future<void> _enqueue(
  CheckinQueueDb db,
  String eventType,
  String t, {
  String appUserId = 'u1',
}) async {
  await db.enqueue(PendingEventsCompanion(
    appUserId: Value(appUserId),
    eventType: Value(eventType),
    lat: const Value(25.0),
    lng: const Value(121.0),
    occurredAtClient: Value(t),
    enqueuedAt: Value(DateTime.now().toIso8601String()),
  ),);
}

SubmitCheckinEventResponse _okResponse(String id) {
  return SubmitCheckinEventResponse(
    event: CheckinEventDto(
      id: id,
      appUserId: 'u1',
      eventType: CheckinEventType.clockIn,
      occurredAtClient: '2026-05-04T08:00:00+08:00',
      occurredAtServer: '2026-05-04T00:00:00Z',
      source: EventSource.app,
      initiatedByKind: EventInitiatorKind.appUser,
      initiatedById: 'u1',
      location: const EventLocation(coordinates: GeoPoint(lat: 25, lng: 121)),
      hasSkewWarning: false,
    ),
    status: const CheckinUserStatusDto(
      appUserId: 'u1',
      status: AppUserCheckinStatus.onSite,
      hasSkewWarning: false,
    ),
  );
}

Object _throw(ApiException e) => e;

class _FakeRepo implements CheckinRepository {
  final List<Object> responses = <Object>[];
  int calls = 0;

  @override
  Future<SubmitCheckinEventResponse> submit(
    SubmitCheckinEventRequest req,
  ) async {
    calls++;
    final next = responses.removeAt(0);
    if (next is ApiException) throw next;
    return next as SubmitCheckinEventResponse;
  }

  @override
  Future<CheckinUserStatusDto> status() async => throw UnimplementedError();

  @override
  Future<List<CheckinEventDto>> events({String? before, int limit = 50}) async =>
      <CheckinEventDto>[];
}

