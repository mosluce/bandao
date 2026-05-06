import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';

import 'package:bandao_app/features/checkin/data/checkin_queue_db.dart';
import 'package:bandao_app/features/checkin/presentation/queue_chip.dart';
import 'package:bandao_app/features/checkin/state/checkin_queue_provider.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

void main() {
  group('QueueChip', () {
    testWidgets('hidden when queue is empty', (tester) async {
      await _pump(tester, queue: const <QueueRow>[]);
      expect(find.byType(ActionChip), findsNothing);
    });

    testWidgets('shows pending count when only pending rows', (tester) async {
      await _pump(tester, queue: <QueueRow>[
        _row(id: 1, status: 'pending'),
        _row(id: 2, status: 'pending'),
        _row(id: 3, status: 'pending'),
      ],);
      expect(find.text('待送出 3 筆'), findsOneWidget);
    });

    testWidgets('shows sending label when a row is in flight', (tester) async {
      await _pump(tester, queue: <QueueRow>[
        _row(id: 1, status: 'sending'),
      ],);
      expect(find.text('送出中'), findsOneWidget);
    });

    testWidgets('shows mixed label for pending + failed', (tester) async {
      await _pump(tester, queue: <QueueRow>[
        _row(id: 1, status: 'pending'),
        _row(id: 2, status: 'pending'),
        _row(id: 3, status: 'failed'),
      ],);
      expect(find.textContaining('待送出 2'), findsOneWidget);
      expect(find.textContaining('1 筆失敗'), findsOneWidget);
    });
  });
}

QueueRow _row({required int id, required String status}) {
  return PendingEvent(
    id: id,
    appUserId: 'u1',
    eventType: 'clock_in',
    lat: 25.0,
    lng: 121.0,
    accuracy: null,
    manualLabel: null,
    occurredAtClient: '2026-05-04T08:00:00+08:00',
    status: status,
    attempts: 0,
    lastErrorCode: null,
    lastErrorMessage: null,
    lastAttemptAt: null,
    enqueuedAt: '2026-05-04T08:00:00.000',
  );
}

Future<void> _pump(WidgetTester tester, {required List<QueueRow> queue}) async {
  final router = GoRouter(
    initialLocation: '/',
    routes: <GoRoute>[
      GoRoute(path: '/', builder: (_, __) => const Scaffold(body: QueueChip())),
      GoRoute(
        path: '/history',
        builder: (_, __) => const Scaffold(body: Text('history')),
      ),
    ],
  );

  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        checkinQueueProvider.overrideWith(
          (ref) => Stream<List<QueueRow>>.value(queue),
        ),
      ],
      child: MaterialApp.router(
        routerConfig: router,
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
  await tester.pumpAndSettle();
}
