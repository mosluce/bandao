import 'package:drift/drift.dart' show Value;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/features/checkin/data/checkin_queue_db.dart';

void main() {
  late CheckinQueueDb db;

  setUp(() {
    db = CheckinQueueDb.forTesting(NativeDatabase.memory());
  });

  tearDown(() async {
    await db.close();
  });

  test('enqueue then pickOldestPending returns the inserted row', () async {
    await _enqueue(db, eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    final row = await db.pickOldestPending();
    expect(row, isNotNull);
    expect(row!.eventType, 'clock_in');
    expect(row.status, 'pending');
    expect(row.attempts, 0);
  });

  test('pickOldestPending picks the earliest occurred_at_client', () async {
    await _enqueue(db, eventType: 'clock_in', t: '2026-05-04T10:00:00+08:00');
    await _enqueue(db, eventType: 'transfer_out', t: '2026-05-04T08:00:00+08:00');
    await _enqueue(db, eventType: 'transfer_in', t: '2026-05-04T09:00:00+08:00');
    final row = await db.pickOldestPending();
    expect(row!.eventType, 'transfer_out');
  });

  test('markSending advances state and increments attempts', () async {
    await _enqueue(db, eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    final row = await db.pickOldestPending();
    await db.markSending(row!.id);

    final inFlight = await db.findInFlight();
    expect(inFlight, isNotNull);
    expect(inFlight!.status, 'sending');
    expect(inFlight.attempts, 1);
    expect(inFlight.lastAttemptAt, isNotNull);
  });

  test('markFailed records error code and moves out of pending', () async {
    await _enqueue(db, eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    final row = await db.pickOldestPending();
    await db.markFailed(
      row!.id,
      errorCode: 'INVALID_TRANSITION',
      errorMessage: 'cannot clock_in from on_site',
    );

    final next = await db.pickOldestPending();
    expect(next, isNull);

    final all = await db.allForUser('u1');
    expect(all.first.status, 'failed');
    expect(all.first.lastErrorCode, 'INVALID_TRANSITION');
    expect(all.first.lastErrorMessage, 'cannot clock_in from on_site');
  });

  test('deleteRow removes the row', () async {
    await _enqueue(db, eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    final row = await db.pickOldestPending();
    await db.deleteRow(row!.id);
    expect(await db.pickOldestPending(), isNull);
  });

  test('wipeForOtherUsers preserves matching rows, deletes others', () async {
    await _enqueue(db, appUserId: 'u1', eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    await _enqueue(db, appUserId: 'u2', eventType: 'clock_in', t: '2026-05-04T09:00:00+08:00');
    await _enqueue(db, appUserId: 'u2', eventType: 'clock_out', t: '2026-05-04T17:00:00+08:00');

    final deleted = await db.wipeForOtherUsers('u1');
    expect(deleted, 2);

    final remaining = await db.allForUser('u1');
    expect(remaining.length, 1);
    expect(remaining.first.appUserId, 'u1');
  });

  test('wipeForOtherUsers returns 0 when nothing to wipe', () async {
    await _enqueue(db, appUserId: 'u1', eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    final deleted = await db.wipeForOtherUsers('u1');
    expect(deleted, 0);
  });

  test('watchAll emits queue changes for the matching user', () async {
    final stream = db.watchAll('u1');
    final emissions = <int>[];
    final sub = stream.listen((rows) => emissions.add(rows.length));
    await Future<void>.delayed(const Duration(milliseconds: 10));
    await _enqueue(db, appUserId: 'u1', eventType: 'clock_in', t: '2026-05-04T08:00:00+08:00');
    await Future<void>.delayed(const Duration(milliseconds: 10));
    await sub.cancel();
    expect(emissions, contains(0));
    expect(emissions.last, 1);
  });

  group('pending_location_pings', () {
    test('enqueueLocationPing inserts and pickPendingLocationBatch returns oldest first', () async {
      await _enqueueLocation(db, t: '2026-05-04T09:00:00+08:00');
      await _enqueueLocation(db, t: '2026-05-04T08:00:00+08:00');
      await _enqueueLocation(db, t: '2026-05-04T08:30:00+08:00');

      final batch = await db.pickPendingLocationBatch(10);
      expect(batch, hasLength(3));
      expect(batch[0].occurredAtClient, '2026-05-04T08:00:00+08:00');
      expect(batch[1].occurredAtClient, '2026-05-04T08:30:00+08:00');
      expect(batch[2].occurredAtClient, '2026-05-04T09:00:00+08:00');
    });

    test('pickPendingLocationBatch caps to max', () async {
      for (var i = 0; i < 5; i++) {
        await _enqueueLocation(
          db,
          t: '2026-05-04T08:0$i:00+08:00',
        );
      }
      final batch = await db.pickPendingLocationBatch(3);
      expect(batch, hasLength(3));
    });

    test('pickPendingLocationBatch ignores rows not in pending', () async {
      await _enqueueLocation(db, t: '2026-05-04T08:00:00+08:00');
      final initial = await db.pickPendingLocationBatch(10);
      await db.markLocationSending(initial.map((r) => r.id).toList());

      final after = await db.pickPendingLocationBatch(10);
      expect(after, isEmpty);
    });

    test('markLocationSending advances state and increments attempts', () async {
      await _enqueueLocation(db, t: '2026-05-04T08:00:00+08:00');
      final initial = await db.pickPendingLocationBatch(10);
      await db.markLocationSending(initial.map((r) => r.id).toList());

      // Re-read via a raw select since pick filters to pending.
      final all = await db.select(db.pendingLocationPings).get();
      expect(all.first.status, 'sending');
      expect(all.first.attempts, 1);
      expect(all.first.lastAttemptAt, isNotNull);
    });

    test('markLocationPending returns rows to pending with error', () async {
      await _enqueueLocation(db, t: '2026-05-04T08:00:00+08:00');
      final initial = await db.pickPendingLocationBatch(10);
      final ids = initial.map((r) => r.id).toList();
      await db.markLocationSending(ids);
      await db.markLocationPending(
        ids,
        lastErrorCode: 'INTERNAL',
        lastErrorMessage: 'oops',
      );

      final pending = await db.pickPendingLocationBatch(10);
      expect(pending, hasLength(1));
      expect(pending.first.status, 'pending');
      expect(pending.first.lastErrorCode, 'INTERNAL');
      expect(pending.first.lastErrorMessage, 'oops');
    });

    test('markLocationFailed moves single row out of pending', () async {
      await _enqueueLocation(db, t: '2026-05-04T08:00:00+08:00');
      final initial = await db.pickPendingLocationBatch(10);
      await db.markLocationFailed(
        initial.first.id,
        errorCode: 'INVALID_PING_TIMESTAMP',
        errorMessage: 'too old',
      );

      final pending = await db.pickPendingLocationBatch(10);
      expect(pending, isEmpty);

      final all = await db.select(db.pendingLocationPings).get();
      expect(all.first.status, 'failed');
      expect(all.first.lastErrorCode, 'INVALID_PING_TIMESTAMP');
    });

    test('deleteLocationPings removes specified rows', () async {
      await _enqueueLocation(db, t: '2026-05-04T08:00:00+08:00');
      await _enqueueLocation(db, t: '2026-05-04T08:01:00+08:00');
      await _enqueueLocation(db, t: '2026-05-04T08:02:00+08:00');

      final batch = await db.pickPendingLocationBatch(10);
      await db.deleteLocationPings([batch[0].id, batch[2].id]);

      final remaining = await db.pickPendingLocationBatch(10);
      expect(remaining, hasLength(1));
      expect(remaining.first.occurredAtClient, '2026-05-04T08:01:00+08:00');
    });

    test('pendingLocationCountForUser counts only matching user', () async {
      await _enqueueLocation(db, appUserId: 'u1', t: '2026-05-04T08:00:00+08:00');
      await _enqueueLocation(db, appUserId: 'u1', t: '2026-05-04T08:01:00+08:00');
      await _enqueueLocation(db, appUserId: 'u2', t: '2026-05-04T08:02:00+08:00');

      expect(await db.pendingLocationCountForUser('u1'), 2);
      expect(await db.pendingLocationCountForUser('u2'), 1);
      expect(await db.pendingLocationCountForUser('u3'), 0);
    });

    test('latestPendingLocationEnqueuedAt returns the newest row', () async {
      await _enqueueLocation(
        db,
        t: '2026-05-04T08:00:00+08:00',
        enqueuedAt: '2026-05-04T08:00:05+08:00',
      );
      await _enqueueLocation(
        db,
        t: '2026-05-04T08:01:00+08:00',
        enqueuedAt: '2026-05-04T08:01:05+08:00',
      );

      final latest = await db.latestPendingLocationEnqueuedAt();
      expect(latest?.toUtc().toIso8601String(), '2026-05-04T00:01:05.000Z');
    });

    test('wipeLocationPingsForOtherUsers preserves only matching user rows',
        () async {
      await _enqueueLocation(db, appUserId: 'u1', t: '2026-05-04T08:00:00+08:00');
      await _enqueueLocation(db, appUserId: 'u2', t: '2026-05-04T08:01:00+08:00');
      await _enqueueLocation(db, appUserId: 'u2', t: '2026-05-04T08:02:00+08:00');

      final deleted = await db.wipeLocationPingsForOtherUsers('u1');
      expect(deleted, 2);

      final remaining = await db.pickPendingLocationBatch(10);
      expect(remaining, hasLength(1));
      expect(remaining.first.appUserId, 'u1');
    });
  });
}

Future<void> _enqueueLocation(
  CheckinQueueDb db, {
  required String t,
  String appUserId = 'u1',
  String? enqueuedAt,
}) async {
  await db.enqueueLocationPing(PendingLocationPingsCompanion(
    appUserId: Value(appUserId),
    lat: const Value(25.0),
    lng: const Value(121.0),
    occurredAtClient: Value(t),
    enqueuedAt: Value(enqueuedAt ?? t),
  ),);
}

Future<void> _enqueue(
  CheckinQueueDb db, {
  required String eventType,
  required String t,
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
