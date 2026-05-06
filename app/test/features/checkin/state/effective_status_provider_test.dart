import 'package:drift/drift.dart' show Value;
import 'package:drift/native.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/checkin_event.dart';
import 'package:bandao_app/core/api/models/checkin_status.dart';
import 'package:bandao_app/features/checkin/data/checkin_queue_db.dart';
import 'package:bandao_app/features/checkin/state/effective_status_provider.dart';

void main() {
  group('applyTransition', () {
    test('off_duty + clock_in -> on_site', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.offDuty,
          CheckinEventType.clockIn,
        ),
        AppUserCheckinStatus.onSite,
      );
    });

    test('on_site + clock_out -> off_duty', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.onSite,
          CheckinEventType.clockOut,
        ),
        AppUserCheckinStatus.offDuty,
      );
    });

    test('on_site + transfer_out -> in_transit', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.onSite,
          CheckinEventType.transferOut,
        ),
        AppUserCheckinStatus.inTransit,
      );
    });

    test('in_transit + transfer_in -> on_site', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.inTransit,
          CheckinEventType.transferIn,
        ),
        AppUserCheckinStatus.onSite,
      );
    });

    test('in_transit + clock_out -> off_duty', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.inTransit,
          CheckinEventType.clockOut,
        ),
        AppUserCheckinStatus.offDuty,
      );
    });

    test('off_duty + clock_out -> null (illegal)', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.offDuty,
          CheckinEventType.clockOut,
        ),
        isNull,
      );
    });

    test('on_site + clock_in -> null (illegal)', () {
      expect(
        applyTransition(
          AppUserCheckinStatus.onSite,
          CheckinEventType.clockIn,
        ),
        isNull,
      );
    });
  });

  group('reduceEffectiveStatus', () {
    late CheckinQueueDb db;

    setUp(() {
      db = CheckinQueueDb.forTesting(NativeDatabase.memory());
    });

    tearDown(() async {
      await db.close();
    });

    test('empty queue collapses to server status', () async {
      final status = const CheckinUserStatusDto(
        appUserId: 'u1',
        status: AppUserCheckinStatus.onSite,
        currentShiftStartedAt: '2026-05-04T08:00:00+08:00',
        hasSkewWarning: false,
      );
      final eff = reduceEffectiveStatus(serverStatus: status, queue: []);
      expect(eff.status, AppUserCheckinStatus.onSite);
      expect(eff.currentShiftStartedAt, '2026-05-04T08:00:00+08:00');
      expect(eff.hasPendingTransition, isFalse);
    });

    test('null server status defaults to off_duty', () {
      final eff = reduceEffectiveStatus(serverStatus: null, queue: []);
      expect(eff.status, AppUserCheckinStatus.offDuty);
      expect(eff.currentShiftStartedAt, isNull);
    });

    test('pending clock_in over off_duty -> optimistic on_site', () async {
      final status = _server(AppUserCheckinStatus.offDuty);
      await _enqueue(db, eventType: 'clock_in', occurredAtClient: '2026-05-04T08:00:00+08:00');
      final rows = await db.allForUser('u1');

      final eff = reduceEffectiveStatus(serverStatus: status, queue: rows);
      expect(eff.status, AppUserCheckinStatus.onSite);
      expect(eff.currentShiftStartedAt, '2026-05-04T08:00:00+08:00');
      expect(eff.hasPendingTransition, isTrue);
    });

    test('failed clock_in is excluded — rolls back to server status', () async {
      final status = _server(AppUserCheckinStatus.offDuty);
      await _enqueue(
        db,
        eventType: 'clock_in',
        occurredAtClient: '2026-05-04T08:00:00+08:00',
        status: 'failed',
      );
      final rows = await db.allForUser('u1');
      final eff = reduceEffectiveStatus(serverStatus: status, queue: rows);
      expect(eff.status, AppUserCheckinStatus.offDuty);
      expect(eff.hasPendingTransition, isFalse);
    });

    test('multi-event sequence: clock_in -> transfer_out -> transfer_in', () async {
      final status = _server(AppUserCheckinStatus.offDuty);
      await _enqueue(db, eventType: 'clock_in', occurredAtClient: '2026-05-04T08:00:00+08:00');
      await _enqueue(db, eventType: 'transfer_out', occurredAtClient: '2026-05-04T10:00:00+08:00');
      await _enqueue(db, eventType: 'transfer_in', occurredAtClient: '2026-05-04T11:00:00+08:00');
      final rows = await db.allForUser('u1');
      final eff = reduceEffectiveStatus(serverStatus: status, queue: rows);
      expect(eff.status, AppUserCheckinStatus.onSite);
    });

    test('failed clock_in then later valid clock_in still optimistic', () async {
      final status = _server(AppUserCheckinStatus.offDuty);
      await _enqueue(
        db,
        eventType: 'clock_in',
        occurredAtClient: '2026-05-04T08:00:00+08:00',
        status: 'failed',
      );
      await _enqueue(
        db,
        eventType: 'clock_in',
        occurredAtClient: '2026-05-04T09:00:00+08:00',
      );
      final rows = await db.allForUser('u1');
      final eff = reduceEffectiveStatus(serverStatus: status, queue: rows);
      expect(eff.status, AppUserCheckinStatus.onSite);
      expect(eff.currentShiftStartedAt, '2026-05-04T09:00:00+08:00');
    });

    test('sending row contributes to overlay', () async {
      final status = _server(AppUserCheckinStatus.offDuty);
      await _enqueue(
        db,
        eventType: 'clock_in',
        occurredAtClient: '2026-05-04T08:00:00+08:00',
        status: 'sending',
      );
      final rows = await db.allForUser('u1');
      final eff = reduceEffectiveStatus(serverStatus: status, queue: rows);
      expect(eff.status, AppUserCheckinStatus.onSite);
      expect(eff.hasPendingTransition, isTrue);
    });
  });
}

CheckinUserStatusDto _server(AppUserCheckinStatus s) => CheckinUserStatusDto(
      appUserId: 'u1',
      status: s,
      hasSkewWarning: false,
    );

Future<void> _enqueue(
  CheckinQueueDb db, {
  required String eventType,
  required String occurredAtClient,
  String status = 'pending',
  String appUserId = 'u1',
}) async {
  await db.enqueue(PendingEventsCompanion(
    appUserId: Value(appUserId),
    eventType: Value(eventType),
    lat: const Value(25.0),
    lng: const Value(121.0),
    occurredAtClient: Value(occurredAtClient),
    status: Value(status),
    enqueuedAt: Value(DateTime.now().toIso8601String()),
  ),);
}
