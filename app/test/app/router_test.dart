import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/app/router.dart';
import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';

import '../helpers/fake_auth_notifier.dart';

void main() {
  group('redirect rules', () {
    testWidgets('unauthenticated -> /login on /', (tester) async {
      await _pumpWithAuth(
        tester,
        const AuthState.unauthenticated(),
        startAt: '/',
      );
      expect(find.text('Login'), findsOneWidget);
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
        expect(find.text('Change password'), findsOneWidget);
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
      expect(find.text('Home'), findsOneWidget);
    });

    testWidgets('error -> /login', (tester) async {
      await _pumpWithAuth(
        tester,
        const AuthState.error('boom'),
        startAt: '/',
      );
      expect(find.text('Login'), findsOneWidget);
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
    ],
  );
  addTearDown(container.dispose);

  final router = container.read(routerProvider);
  router.go(startAt);

  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp.router(routerConfig: router),
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
