import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';

import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/checkin_event.dart';
import 'package:argus_app/core/api/models/checkin_status.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/core/api/models/submit_checkin_event.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/features/checkin/data/checkin_queue_db.dart';
import 'package:argus_app/features/checkin/data/checkin_repository.dart';
import 'package:argus_app/features/checkin/presentation/history_screen.dart';
import 'package:argus_app/features/checkin/state/checkin_queue_provider.dart';
import 'package:argus_app/features/checkin/state/checkin_status_provider.dart';
import 'package:argus_app/features/checkin/state/recently_synced_events_provider.dart';
import 'package:argus_app/l10n/app_localizations.dart';

import '../../../helpers/fake_auth_notifier.dart';

void main() {
  group('HistoryScreen', () {
    testWidgets('pending and synced rows render together', (tester) async {
      await _pump(
        tester,
        queue: <QueueRow>[_queueRow(1, 'pending', '2026-05-04T09:30:00+08:00')],
        serverEvents: <CheckinEventDto>[
          _serverEvent('s1', '2026-05-04T07:00:00+08:00'),
        ],
      );
      // Pending badge for the local row
      expect(find.text('待送出'), findsOneWidget);
      // Synced badge for the server row
      expect(find.text('已上傳'), findsOneWidget);
    });

    testWidgets(
        'just-synced event from recently-synced cache stays visible',
        (tester) async {
      await _pump(
        tester,
        queue: const <QueueRow>[],
        serverEvents: const <CheckinEventDto>[],
        recentlySynced: <CheckinEventDto>[
          _serverEvent('e1', '2026-05-04T09:30:00+08:00'),
        ],
      );
      // Row from recently-synced cache should render with synced badge.
      expect(find.text('已上傳'), findsOneWidget);
    });

    testWidgets('failed row exposes dismiss controls', (tester) async {
      await _pump(
        tester,
        queue: <QueueRow>[
          _queueRow(
            1,
            'failed',
            '2026-05-04T09:30:00+08:00',
            errorCode: 'TRANSFER_DISABLED',
            errorMessage: 'transfer disabled',
          ),
        ],
        serverEvents: const <CheckinEventDto>[],
      );
      expect(find.text('失敗'), findsOneWidget);
      expect(find.text('複製細節'), findsOneWidget);
      expect(find.text('關閉'), findsOneWidget);
    });

    testWidgets('load more uses oldest occurred_at_client as `before`',
        (tester) async {
      final repo = _RecordingRepo();
      // Pre-fill 50 events so the footer shows 載入更多.
      final firstPage = <CheckinEventDto>[
        for (var i = 0; i < 50; i++)
          _serverEvent('e$i', '2026-05-04T${(10 - i ~/ 6).toString().padLeft(2, '0')}:00:00+08:00'),
      ];
      repo.responses.addAll([firstPage, <CheckinEventDto>[]]);

      await _pump(
        tester,
        queue: const <QueueRow>[],
        serverEvents: const <CheckinEventDto>[],
        repoOverride: repo,
      );
      // Wait for the initial fetch.
      await tester.pumpAndSettle();
      expect(repo.calls.length, 1);
      expect(repo.calls.first.before, isNull);

      // Trigger load more.
      await tester.scrollUntilVisible(find.text('載入更多'), 200);
      await tester.tap(find.text('載入更多'));
      await tester.pumpAndSettle();

      expect(repo.calls.length, 2);
      // The oldest currently-displayed server row's occurred_at_client.
      expect(
        repo.calls.last.before,
        equals(firstPage.last.occurredAtClient),
      );
    });

    testWidgets('dedupe keeps a single row when server fetch overlaps cache',
        (tester) async {
      final shared = _serverEvent('e1', '2026-05-04T09:30:00+08:00');
      final repo = _RecordingRepo();
      repo.responses.add(<CheckinEventDto>[shared]);

      await _pump(
        tester,
        queue: const <QueueRow>[],
        serverEvents: const <CheckinEventDto>[],
        recentlySynced: <CheckinEventDto>[shared],
        repoOverride: repo,
      );
      await tester.pumpAndSettle();

      // Even though the same event is in both the recently-synced cache
      // and the server fetch result, only one row renders.
      expect(find.text('已上傳'), findsOneWidget);
    });
  });
}

QueueRow _queueRow(
  int id,
  String status,
  String t, {
  String? errorCode,
  String? errorMessage,
}) {
  return PendingEvent(
    id: id,
    appUserId: 'u1',
    eventType: 'clock_in',
    lat: 25.0,
    lng: 121.0,
    accuracy: null,
    manualLabel: null,
    occurredAtClient: t,
    status: status,
    attempts: status == 'failed' ? 1 : 0,
    lastErrorCode: errorCode,
    lastErrorMessage: errorMessage,
    lastAttemptAt: status == 'failed' ? '2026-05-04T09:30:00+08:00' : null,
    enqueuedAt: t,
  );
}

CheckinEventDto _serverEvent(String id, String t) => CheckinEventDto(
      id: id,
      appUserId: 'u1',
      eventType: CheckinEventType.clockIn,
      occurredAtClient: t,
      occurredAtServer: '2026-05-04T00:00:00Z',
      source: EventSource.app,
      initiatedByKind: EventInitiatorKind.appUser,
      initiatedById: 'u1',
      location:
          const EventLocation(coordinates: GeoPoint(lat: 25, lng: 121)),
      hasSkewWarning: false,
    );

Future<void> _pump(
  WidgetTester tester, {
  required List<QueueRow> queue,
  required List<CheckinEventDto> serverEvents,
  List<CheckinEventDto>? recentlySynced,
  _RecordingRepo? repoOverride,
}) async {
  final repo = repoOverride ?? _RecordingRepo();
  if (repoOverride == null) {
    // Default: no pagination needed.
    repo.responses.add(serverEvents);
  }

  final router = GoRouter(
    initialLocation: '/history',
    routes: <GoRoute>[
      GoRoute(path: '/', builder: (_, __) => const Scaffold(body: Text('home'))),
      GoRoute(path: '/history', builder: (_, __) => const HistoryScreen()),
    ],
  );

  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        authProvider.overrideWith(
          () => FakeAuthNotifier(AsyncValue.data(_authedAuthState())),
        ),
        checkinQueueProvider.overrideWith(
          (ref) => Stream<List<QueueRow>>.value(queue),
        ),
        checkinRepositoryProvider.overrideWith((ref) async => repo),
        checkinStatusProvider.overrideWith(_NullStatusNotifier.new),
        if (recentlySynced != null)
          recentlySyncedEventsProvider.overrideWith(
            () => _SeededRecentlySyncedNotifier(recentlySynced),
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

AuthState _authedAuthState() => const AuthState.authenticated(
      user: AppUser(
        id: 'u1',
        username: 'alice',
        displayName: 'Alice',
        status: AppUserStatus.active,
        needsPasswordChange: false,
        createdAt: '2025-01-01T00:00:00Z',
      ),
      org: Org(
        id: 'o1',
        name: 'Acme',
        code: 'ABCDEFGHIJ',
        ownerId: 'u1',
        timezone: 'Asia/Taipei',
        checkin: OrgCheckin(transferEnabled: true),
      ),
      needsPasswordChange: false,
    );

class _RecordingRepo implements CheckinRepository {
  final List<({String? before, int limit})> calls = [];
  final List<List<CheckinEventDto>> responses = [];

  @override
  Future<List<CheckinEventDto>> events({String? before, int limit = 50}) async {
    calls.add((before: before, limit: limit));
    if (responses.isEmpty) return const [];
    return responses.removeAt(0);
  }

  @override
  Future<SubmitCheckinEventResponse> submit(
    SubmitCheckinEventRequest req,
  ) async => throw UnimplementedError();

  @override
  Future<CheckinUserStatusDto> status() async => throw UnimplementedError();
}

class _NullStatusNotifier extends CheckinStatusNotifier {
  @override
  Future<CheckinUserStatusDto?> build() async => null;
}

class _SeededRecentlySyncedNotifier extends RecentlySyncedEventsNotifier {
  _SeededRecentlySyncedNotifier(this._seed);
  final List<CheckinEventDto> _seed;

  @override
  List<CheckinEventDto> build() => _seed;
}
