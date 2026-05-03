import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/app/router.dart';
import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/core/storage/secure_storage.dart';
import 'package:argus_app/features/auth/presentation/dev_server_config_screen.dart';
import 'package:argus_app/features/auth/presentation/force_password_change_screen.dart';
import 'package:argus_app/features/auth/presentation/home_screen.dart';
import 'package:argus_app/features/auth/presentation/login_screen.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/l10n/app_localizations.dart';

import '../helpers/fake_auth_notifier.dart';
import '../helpers/fake_secure_storage.dart';

void main() {
  group('redirect rules', () {
    testWidgets('unauthenticated -> /login on /', (tester) async {
      await _pumpWithAuth(
        tester,
        const AuthState.unauthenticated(),
        startAt: '/',
      );
      expect(find.byType(LoginScreen), findsOneWidget);
    });

    testWidgets(
      'authenticated && needsPasswordChange -> /force-change-password',
      (tester) async {
        await _pumpWithAuth(
          tester,
          AuthState.authenticated(
            user: _user,
            org: _org,
            needsPasswordChange: true,
          ),
          startAt: '/',
        );
        expect(find.byType(ForcePasswordChangeScreen), findsOneWidget);
      },
    );

    testWidgets('authenticated && !needsPasswordChange -> /', (tester) async {
      await _pumpWithAuth(
        tester,
        AuthState.authenticated(
          user: _user,
          org: _org,
          needsPasswordChange: false,
        ),
        startAt: '/login',
      );
      expect(find.byType(HomeScreen), findsOneWidget);
    });

    testWidgets('error -> /login', (tester) async {
      await _pumpWithAuth(
        tester,
        const AuthState.error('boom'),
        startAt: '/',
      );
      expect(find.byType(LoginScreen), findsOneWidget);
    });

    testWidgets('dev menu route reachable while unauthenticated',
        (tester) async {
      await _pumpWithAuth(
        tester,
        const AuthState.unauthenticated(),
        startAt: '/dev-server-config',
      );
      expect(find.byType(DevServerConfigScreen), findsOneWidget);
    });
  });
}

Future<void> _pumpWithAuth(
  WidgetTester tester,
  AuthState initial, {
  required String startAt,
}) async {
  final container = ProviderContainer(
    overrides: <Override>[
      authProvider
          .overrideWith(() => FakeAuthNotifier(AsyncValue.data(initial))),
      secureStorageProvider.overrideWithValue(FakeSecureStorage()),
    ],
  );
  addTearDown(container.dispose);

  final router = container.read(routerProvider);
  router.go(startAt);

  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
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

final AppUser _user = const AppUser(
  id: 'u1',
  username: 'alice',
  displayName: 'Alice Chen',
  status: AppUserStatus.active,
  needsPasswordChange: false,
  createdAt: '2025-01-01T00:00:00Z',
);

final Org _org = const Org(
  id: 'o1',
  name: 'Acme Corp',
  code: 'ABCDEFGHIJ',
  ownerId: 'u1',
  timezone: 'Asia/Taipei',
  checkin: OrgCheckin(transferEnabled: true),
);
