import 'package:drift/native.dart';
import 'package:drift/drift.dart' show Value;
import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/api/models/checkin_status.dart';
import 'package:bandao_app/core/storage/secure_storage.dart';
import 'package:bandao_app/features/checkin/data/checkin_queue_db.dart';
import 'package:bandao_app/features/checkin/presentation/tracking_recovery_banner.dart';
import 'package:bandao_app/features/checkin/state/checkin_status_provider.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

import '../../../helpers/fake_secure_storage.dart';

void main() {
  group('TrackingRecoveryBanner', () {
    testWidgets('hidden when off_duty', (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.offDuty,
        lastCleanStop: null,
      );
      expect(find.text('定位追蹤上次中斷過，已恢復記錄。'), findsNothing);
    });

    testWidgets('visible when on_site and last_clean_stop missing',
        (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.onSite,
        lastCleanStop: null,
      );
      expect(find.text('定位追蹤上次中斷過，已恢復記錄。'), findsOneWidget);
    });

    testWidgets('hidden when last_clean_stop is fresh (no pending pings)',
        (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.onSite,
        lastCleanStop: DateTime(2026, 5, 4, 8),
      );
      expect(find.text('定位追蹤上次中斷過，已恢復記錄。'), findsNothing);
    });

    testWidgets('visible when last_clean_stop is older than latest enqueued',
        (tester) async {
      final db = CheckinQueueDb.forTesting(NativeDatabase.memory());
      addTearDown(() async => db.close());
      await db.enqueueLocationPing(PendingLocationPingsCompanion(
        appUserId: const Value('u1'),
        lat: const Value(25.0),
        lng: const Value(121.0),
        occurredAtClient: const Value('2026-05-04T09:00:00+08:00'),
        enqueuedAt: const Value('2026-05-04T09:00:00Z'),
      ),);

      await _pump(
        tester,
        status: AppUserCheckinStatus.onSite,
        lastCleanStop: DateTime.utc(2026, 5, 4, 8),
        db: db,
      );
      expect(find.text('定位追蹤上次中斷過，已恢復記錄。'), findsOneWidget);
    });

    testWidgets('dismiss button hides banner', (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.onSite,
        lastCleanStop: null,
      );
      expect(find.text('定位追蹤上次中斷過，已恢復記錄。'), findsOneWidget);

      await tester.tap(find.text('了解'));
      await tester.pumpAndSettle();
      expect(find.text('定位追蹤上次中斷過，已恢復記錄。'), findsNothing);
    });
  });
}

Future<void> _pump(
  WidgetTester tester, {
  required AppUserCheckinStatus status,
  required DateTime? lastCleanStop,
  CheckinQueueDb? db,
}) async {
  final storage = FakeSecureStorage();
  if (lastCleanStop != null) {
    await storage.writeLocationTrackingLastCleanStop(lastCleanStop);
  }
  final database = db ?? CheckinQueueDb.forTesting(NativeDatabase.memory());
  if (db == null) addTearDown(() async => database.close());

  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        secureStorageProvider.overrideWithValue(storage),
        checkinQueueDbProvider.overrideWithValue(database),
        checkinStatusProvider.overrideWith(() => _FakeStatusNotifier(status)),
      ],
      child: MaterialApp(
        home: const Scaffold(
          body: Column(
            children: <Widget>[TrackingRecoveryBanner()],
          ),
        ),
        locale: const Locale('zh', 'TW'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
      ),
    ),
  );
  // Two pumps: one for the post-frame callback to fire, another for the
  // async _evaluate to complete and call setState.
  await tester.pumpAndSettle();
}

class _FakeStatusNotifier extends CheckinStatusNotifier {
  _FakeStatusNotifier(this._status);
  final AppUserCheckinStatus _status;

  @override
  Future<CheckinUserStatusDto?> build() async {
    return CheckinUserStatusDto(
      appUserId: 'u1',
      status: _status,
      hasSkewWarning: false,
    );
  }
}
