import 'dart:io';

import 'package:drift/drift.dart';
import 'package:drift/native.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:path/path.dart' as p;
import 'package:path_provider/path_provider.dart';

part 'checkin_queue_db.g.dart';

@TableIndex(name: 'idx_pending_status_time', columns: {#status, #occurredAtClient})
class PendingEvents extends Table {
  IntColumn get id => integer().autoIncrement()();
  TextColumn get appUserId => text()();
  TextColumn get eventType => text()();
  RealColumn get lat => real()();
  RealColumn get lng => real()();
  RealColumn get accuracy => real().nullable()();
  TextColumn get manualLabel => text().nullable()();
  TextColumn get occurredAtClient => text()();
  TextColumn get status => text().withDefault(const Constant('pending'))();
  IntColumn get attempts => integer().withDefault(const Constant(0))();
  TextColumn get lastErrorCode => text().nullable()();
  TextColumn get lastErrorMessage => text().nullable()();
  TextColumn get lastAttemptAt => text().nullable()();
  TextColumn get enqueuedAt => text()();
}

@TableIndex(name: 'idx_pending_loc_status_time', columns: {#status, #occurredAtClient})
class PendingLocationPings extends Table {
  IntColumn get id => integer().autoIncrement()();
  TextColumn get appUserId => text()();
  RealColumn get lat => real()();
  RealColumn get lng => real()();
  RealColumn get accuracy => real().nullable()();
  TextColumn get occurredAtClient => text()();
  TextColumn get status => text().withDefault(const Constant('pending'))();
  IntColumn get attempts => integer().withDefault(const Constant(0))();
  TextColumn get lastErrorCode => text().nullable()();
  TextColumn get lastErrorMessage => text().nullable()();
  TextColumn get lastAttemptAt => text().nullable()();
  TextColumn get enqueuedAt => text()();
}

typedef QueueRow = PendingEvent;
typedef LocationPingRow = PendingLocationPing;

@DriftDatabase(tables: [PendingEvents, PendingLocationPings])
class CheckinQueueDb extends _$CheckinQueueDb {
  CheckinQueueDb() : super(_openConnection());
  CheckinQueueDb.forTesting(super.e);

  @override
  int get schemaVersion => 2;

  @override
  MigrationStrategy get migration => MigrationStrategy(
        onCreate: (m) async {
          await m.createAll();
        },
        onUpgrade: (m, from, to) async {
          if (from < 2) {
            // Add the location-pings table introduced in
            // `add-location-tracking-app`. Existing pending_events rows are
            // preserved untouched.
            await m.createTable(pendingLocationPings);
            await m.createIndex(
              Index(
                'idx_pending_loc_status_time',
                'CREATE INDEX idx_pending_loc_status_time '
                'ON pending_location_pings (status, occurred_at_client);',
              ),
            );
          }
        },
      );

  Future<int> enqueue(PendingEventsCompanion row) =>
      into(pendingEvents).insert(row);

  Future<QueueRow?> pickOldestPending() {
    return (select(pendingEvents)
          ..where((t) => t.status.equals('pending'))
          ..orderBy([(t) => OrderingTerm.asc(t.occurredAtClient)])
          ..limit(1))
        .getSingleOrNull();
  }

  Future<QueueRow?> findInFlight() {
    return (select(pendingEvents)
          ..where((t) => t.status.equals('sending'))
          ..limit(1))
        .getSingleOrNull();
  }

  Future<void> markSending(int id) async {
    await customStatement(
      'UPDATE pending_events '
      "SET status = 'sending', attempts = attempts + 1, "
      'last_attempt_at = ? '
      'WHERE id = ?',
      [DateTime.now().toIso8601String(), id],
    );
  }

  Future<void> markPending(
    int id, {
    String? lastErrorCode,
    String? lastErrorMessage,
  }) async {
    await (update(pendingEvents)..where((t) => t.id.equals(id))).write(
      PendingEventsCompanion(
        status: const Value('pending'),
        lastErrorCode: Value(lastErrorCode),
        lastErrorMessage: Value(lastErrorMessage),
      ),
    );
  }

  Future<void> markFailed(
    int id, {
    required String errorCode,
    required String errorMessage,
  }) async {
    await (update(pendingEvents)..where((t) => t.id.equals(id))).write(
      PendingEventsCompanion(
        status: const Value('failed'),
        lastErrorCode: Value(errorCode),
        lastErrorMessage: Value(errorMessage),
        lastAttemptAt: Value(DateTime.now().toIso8601String()),
      ),
    );
  }

  Future<void> deleteRow(int id) async {
    await (delete(pendingEvents)..where((t) => t.id.equals(id))).go();
  }

  Future<int> wipeForOtherUsers(String currentUserId) async {
    return (delete(pendingEvents)
          ..where((t) => t.appUserId.equals(currentUserId).not()))
        .go();
  }

  Stream<List<QueueRow>> watchAll(String forUserId) {
    return (select(pendingEvents)
          ..where((t) => t.appUserId.equals(forUserId))
          ..orderBy([(t) => OrderingTerm.desc(t.occurredAtClient)]))
        .watch();
  }

  Future<List<QueueRow>> allForUser(String forUserId) {
    return (select(pendingEvents)
          ..where((t) => t.appUserId.equals(forUserId))
          ..orderBy([(t) => OrderingTerm.desc(t.occurredAtClient)]))
        .get();
  }

  // ---- pending_location_pings ----

  Future<int> enqueueLocationPing(PendingLocationPingsCompanion row) =>
      into(pendingLocationPings).insert(row);

  Future<List<LocationPingRow>> pickPendingLocationBatch(int max) {
    return (select(pendingLocationPings)
          ..where((t) => t.status.equals('pending'))
          ..orderBy([(t) => OrderingTerm.asc(t.occurredAtClient)])
          ..limit(max.clamp(1, 1000)))
        .get();
  }

  Future<int> pendingLocationCountForUser(String forUserId) async {
    final count = countAll(filter: pendingLocationPings.appUserId.equals(forUserId));
    final row = await (selectOnly(pendingLocationPings)..addColumns([count])).getSingle();
    return row.read(count) ?? 0;
  }

  Future<DateTime?> latestPendingLocationEnqueuedAt() async {
    final row = await (select(pendingLocationPings)
          ..orderBy([(t) => OrderingTerm.desc(t.enqueuedAt)])
          ..limit(1))
        .getSingleOrNull();
    if (row == null) return null;
    return DateTime.tryParse(row.enqueuedAt);
  }

  Future<void> markLocationSending(List<int> ids) async {
    if (ids.isEmpty) return;
    await (update(pendingLocationPings)..where((t) => t.id.isIn(ids))).write(
      PendingLocationPingsCompanion(
        status: const Value('sending'),
        lastAttemptAt: Value(DateTime.now().toIso8601String()),
      ),
    );
    await customStatement(
      'UPDATE pending_location_pings '
      'SET attempts = attempts + 1 '
      'WHERE id IN (${List.filled(ids.length, '?').join(',')})',
      ids,
    );
  }

  Future<void> markLocationPending(
    List<int> ids, {
    String? lastErrorCode,
    String? lastErrorMessage,
  }) async {
    if (ids.isEmpty) return;
    await (update(pendingLocationPings)..where((t) => t.id.isIn(ids))).write(
      PendingLocationPingsCompanion(
        status: const Value('pending'),
        lastErrorCode: Value(lastErrorCode),
        lastErrorMessage: Value(lastErrorMessage),
      ),
    );
  }

  Future<void> markLocationFailed(
    int id, {
    required String errorCode,
    required String errorMessage,
  }) async {
    await (update(pendingLocationPings)..where((t) => t.id.equals(id))).write(
      PendingLocationPingsCompanion(
        status: const Value('failed'),
        lastErrorCode: Value(errorCode),
        lastErrorMessage: Value(errorMessage),
        lastAttemptAt: Value(DateTime.now().toIso8601String()),
      ),
    );
  }

  Future<void> deleteLocationPings(List<int> ids) async {
    if (ids.isEmpty) return;
    await (delete(pendingLocationPings)..where((t) => t.id.isIn(ids))).go();
  }

  Future<void> deleteAllLocationPings() async {
    await delete(pendingLocationPings).go();
  }

  /// Wipe location pings for users other than the current one — same
  /// device-handover semantics as `wipeForOtherUsers` for events.
  Future<int> wipeLocationPingsForOtherUsers(String currentUserId) async {
    return (delete(pendingLocationPings)
          ..where((t) => t.appUserId.equals(currentUserId).not()))
        .go();
  }

  Stream<List<LocationPingRow>> watchAllLocationPings() {
    return select(pendingLocationPings).watch();
  }
}

LazyDatabase _openConnection() {
  return LazyDatabase(() async {
    final dir = await getApplicationDocumentsDirectory();
    final file = File(p.join(dir.path, 'bandao_checkin_queue.sqlite'));
    return NativeDatabase.createInBackground(file);
  });
}

final checkinQueueDbProvider = Provider<CheckinQueueDb>((ref) {
  final db = CheckinQueueDb();
  ref.onDispose(db.close);
  return db;
});
