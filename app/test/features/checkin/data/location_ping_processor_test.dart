import 'package:drift/drift.dart' show Value;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/api_error.dart';
import 'package:bandao_app/core/api/models/location_ping.dart';
import 'package:bandao_app/features/checkin/data/checkin_queue_db.dart';
import 'package:bandao_app/features/checkin/data/location_ping_processor.dart';
import 'package:bandao_app/features/checkin/data/location_repository.dart';

void main() {
  group('LocationPingProcessor', () {
    late CheckinQueueDb db;
    late _FakeRepo repo;
    bool authExpiredCalled = false;
    bool trackingDisabledCalled = false;
    DateTime fixedNow = DateTime(2026, 5, 5, 9);

    setUp(() {
      db = CheckinQueueDb.forTesting(NativeDatabase.memory());
      repo = _FakeRepo();
      authExpiredCalled = false;
      trackingDisabledCalled = false;
      fixedNow = DateTime(2026, 5, 5, 9);
      addTearDown(() async {
        await db.close();
      });
    });

    LocationPingProcessor build({bool online = true}) {
      return LocationPingProcessor(
        db: db,
        repo: () async => repo,
        isOnline: () => online,
        onAuthExpired: () async {
          authExpiredCalled = true;
        },
        onTrackingDisabled: () async {
          trackingDisabledCalled = true;
        },
        now: () => fixedNow,
      );
    }

    test('skips flush when below 30-row threshold and no time elapsed',
        () async {
      await _enqueueN(db, 10);
      repo.responses.add(_okResponse(10));
      await build().tick();
      expect(repo.calls, 0);
      expect(await db.pickPendingLocationBatch(100), hasLength(10));
    });

    test('30-row threshold triggers flush', () async {
      await _enqueueN(db, 30);
      repo.responses.add(_okResponse(30));
      await build().tick();
      expect(repo.calls, 1);
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });

    test('time-threshold triggers flush even with sparse pings', () async {
      // First flush at fixedNow.
      await _enqueueN(db, 30);
      repo.responses.add(_okResponse(30));
      final processor = build();
      await processor.tick();
      expect(repo.calls, 1);

      // Add 5 more rows. Below 30 — no flush yet, even though it's seconds
      // later.
      await _enqueueN(db, 5);
      repo.responses.add(_okResponse(5));
      fixedNow = fixedNow.add(const Duration(minutes: 1));
      await processor.tick();
      expect(repo.calls, 1, reason: '1-minute elapsed should not yet flush');

      // 5 minutes elapse → flush even with only 5 pings.
      fixedNow = fixedNow.add(const Duration(minutes: 5));
      await processor.tick();
      expect(repo.calls, 2);
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });

    test('flushFinal drains regardless of threshold', () async {
      await _enqueueN(db, 5);
      repo.responses.add(_okResponse(5));
      final processor = build();
      processor.requestFinalFlush();
      await processor.tick();
      expect(repo.calls, 1);
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });

    test('rejected[] entries are deleted silently along with accepted', () async {
      await _enqueueN(db, 10);
      repo.responses.add(SubmitLocationPingsResponse(
        acceptedCount: 9,
        rejected: const [
          RejectedPingDto(
            index: 4,
            code: 'INVALID_PING_TIMESTAMP',
            message: 'too old',
          ),
        ],
      ),);
      final processor = build();
      processor.requestFinalFlush();
      await processor.tick();
      // All 10 in-flight rows deleted (9 accepted + 1 rejected silently).
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });

    test('5xx returns rows to pending', () async {
      await _enqueueN(db, 30);
      repo.responses.add(_throw(ApiException(
        status: 500,
        code: 'INTERNAL',
        message: 'oops',
      ),),);
      await build().tick();
      final remaining = await db.pickPendingLocationBatch(100);
      expect(remaining, hasLength(30));
      expect(remaining.first.attempts, 1);
    });

    test('LOCATION_TRACKING_DISABLED stops tracker + deletes in-flight',
        () async {
      await _enqueueN(db, 30);
      repo.responses.add(_throw(ApiException(
        status: 403,
        code: ApiErrorCode.locationTrackingDisabled,
        message: 'org disabled',
      ),),);
      await build().tick();
      expect(trackingDisabledCalled, isTrue);
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });

    test('401 signals auth expired + deletes in-flight', () async {
      await _enqueueN(db, 30);
      repo.responses.add(_throw(ApiException.unauthorized()));
      await build().tick();
      expect(authExpiredCalled, isTrue);
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });

    test('offline skips flush', () async {
      await _enqueueN(db, 50);
      await build(online: false).tick();
      expect(repo.calls, 0);
      expect(await db.pickPendingLocationBatch(100), hasLength(50));
    });

    test('batch cap of 100 with recursion drains the rest', () async {
      await _enqueueN(db, 130);
      repo.responses.add(_okResponse(100));
      repo.responses.add(_okResponse(30));
      final processor = build();
      processor.requestFinalFlush();
      await processor.tick();
      expect(repo.calls, 2);
      expect(await db.pickPendingLocationBatch(100), isEmpty);
    });
  });

  group('LocationPingProcessor.nextDelay', () {
    test('walks the schedule', () {
      expect(LocationPingProcessor.nextDelay(1), const Duration(seconds: 1));
      expect(LocationPingProcessor.nextDelay(3), const Duration(seconds: 4));
      expect(LocationPingProcessor.nextDelay(6), const Duration(seconds: 30));
      expect(LocationPingProcessor.nextDelay(99), const Duration(seconds: 30));
    });
  });
}

Future<void> _enqueueN(CheckinQueueDb db, int n) async {
  for (var i = 0; i < n; i++) {
    final t = DateTime(2026, 5, 5, 8).add(Duration(seconds: i));
    await db.enqueueLocationPing(PendingLocationPingsCompanion(
      appUserId: const Value('u1'),
      lat: const Value(25.0),
      lng: const Value(121.0),
      occurredAtClient: Value(t.toIso8601String()),
      enqueuedAt: Value(t.toIso8601String()),
    ),);
  }
}

SubmitLocationPingsResponse _okResponse(int n) => SubmitLocationPingsResponse(
      acceptedCount: n,
      rejected: const [],
    );

Object _throw(ApiException e) => e;

class _FakeRepo implements LocationRepository {
  final List<Object> responses = [];
  int calls = 0;

  @override
  Future<SubmitLocationPingsResponse> submitBatch(
    SubmitLocationPingsRequest req,
  ) async {
    calls++;
    final next = responses.removeAt(0);
    if (next is ApiException) throw next;
    return next as SubmitLocationPingsResponse;
  }
}
