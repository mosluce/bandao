import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';
import 'package:go_router/go_router.dart';

import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/checkin_status.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/features/auth/presentation/home_screen.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/features/checkin/data/checkin_queue_db.dart';
import 'package:argus_app/features/checkin/state/checkin_queue_provider.dart';
import 'package:argus_app/features/checkin/state/checkin_status_provider.dart';
import 'package:argus_app/features/checkin/state/effective_status_provider.dart';
import 'package:argus_app/features/checkin/state/location_permission_provider.dart';
import 'package:argus_app/l10n/app_localizations.dart';

import '../../../helpers/fake_auth_notifier.dart';

void main() {
  group('HomeScreen logout', () {
    testWidgets('empty queue logs out without dialog', (tester) async {
      final fake = await _pump(tester, queue: const <QueueRow>[]);
      await tester.tap(find.byIcon(Icons.more_vert));
      await tester.pumpAndSettle();
      await tester.tap(find.text('登出'));
      await tester.pumpAndSettle();
      // Dialog should NOT appear; logout fired immediately.
      expect(find.text('確定要登出嗎？'), findsNothing);
      expect(fake.logoutCalls, 1);
    });

    testWidgets('non-empty queue surfaces confirm dialog', (tester) async {
      await _pump(tester, queue: <QueueRow>[
        _row(1, 'pending'),
        _row(2, 'failed'),
      ],);
      await tester.tap(find.byIcon(Icons.more_vert));
      await tester.pumpAndSettle();
      await tester.tap(find.text('登出'));
      await tester.pumpAndSettle();
      expect(find.text('確定要登出嗎？'), findsOneWidget);
      expect(find.textContaining('2 筆事件未處理'), findsOneWidget);
      expect(find.text('取消'), findsOneWidget);
      expect(find.text('仍要登出'), findsOneWidget);
    });

    testWidgets('cancel keeps session intact', (tester) async {
      final fake = await _pump(tester, queue: <QueueRow>[_row(1, 'pending')]);
      await tester.tap(find.byIcon(Icons.more_vert));
      await tester.pumpAndSettle();
      await tester.tap(find.text('登出'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(fake.logoutCalls, 0);
    });

    testWidgets('confirm proceeds with logout', (tester) async {
      final fake = await _pump(tester, queue: <QueueRow>[_row(1, 'pending')]);
      await tester.tap(find.byIcon(Icons.more_vert));
      await tester.pumpAndSettle();
      await tester.tap(find.text('登出'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('仍要登出'));
      await tester.pumpAndSettle();
      expect(fake.logoutCalls, 1);
    });
  });
}

QueueRow _row(int id, String status) {
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

Future<_LogoutCountingNotifier> _pump(
  WidgetTester tester, {
  required List<QueueRow> queue,
}) async {
  final fake = _LogoutCountingNotifier(_authedAuthState());
  final router = GoRouter(
    initialLocation: '/',
    routes: <GoRoute>[
      GoRoute(path: '/', builder: (_, __) => const HomeScreen()),
      GoRoute(
        path: '/login',
        builder: (_, __) => const Scaffold(body: Text('login')),
      ),
      GoRoute(
        path: '/history',
        builder: (_, __) => const Scaffold(body: Text('history')),
      ),
    ],
  );

  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        authProvider.overrideWith(() => fake),
        checkinQueueProvider.overrideWith(
          (ref) => Stream<List<QueueRow>>.value(queue),
        ),
        // Stub out checkin status so the home doesn't kick a real network
        // fetch for /app/checkin/status.
        checkinStatusProvider.overrideWith(_NullStatusNotifier.new),
        // Permission and effective status fixed to safe defaults.
        locationPermissionProvider.overrideWith(_NotDeterminedNotifier.new),
        effectiveStatusProvider.overrideWithValue(EffectiveStatus.offDuty),
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
  return fake;
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

class _LogoutCountingNotifier extends FakeAuthNotifier {
  _LogoutCountingNotifier(AuthState initial)
      : super(AsyncValue<AuthState>.data(initial));

  int logoutCalls = 0;

  @override
  Future<void> logout() async {
    logoutCalls++;
  }
}

class _NotDeterminedNotifier extends LocationPermissionNotifier {
  @override
  Future<LocationPermission> build() async => LocationPermission.denied;
}

class _NullStatusNotifier extends CheckinStatusNotifier {
  @override
  Future<CheckinUserStatusDto?> build() async => null;
}
