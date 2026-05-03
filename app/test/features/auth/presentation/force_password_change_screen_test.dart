import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/api_error.dart';
import 'package:argus_app/core/api/models/app_user.dart';
import 'package:argus_app/core/api/models/org.dart';
import 'package:argus_app/features/auth/presentation/force_password_change_screen.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/l10n/app_localizations.dart';

import '../../../helpers/fake_auth_notifier.dart';

void main() {
  testWidgets('submit disabled until current is non-empty and new >= 8',
      (tester) async {
    final notifier = FakeAuthNotifier(
      AsyncValue<AuthState>.data(
        AuthState.authenticated(
          user: _user,
          org: _org,
          needsPasswordChange: true,
        ),
      ),
    );
    await _pump(tester, notifier: notifier);

    final submit = find.byKey(const Key('forceChange.submit'));
    expect(_isEnabled(tester, submit), isFalse);

    await tester.enterText(
      find.byKey(const Key('forceChange.current')),
      'old',
    );
    await tester.pump();
    expect(_isEnabled(tester, submit), isFalse);

    await tester.enterText(
      find.byKey(const Key('forceChange.next')),
      'short',
    );
    await tester.pump();
    expect(_isEnabled(tester, submit), isFalse);

    await tester.enterText(
      find.byKey(const Key('forceChange.next')),
      'longenough',
    );
    await tester.pump();
    expect(_isEnabled(tester, submit), isTrue);
  });

  testWidgets('INVALID_PASSWORD renders friendly Chinese', (tester) async {
    final notifier = FakeAuthNotifier(
      AsyncValue<AuthState>.data(
        AuthState.authenticated(
          user: _user,
          org: _org,
          needsPasswordChange: true,
        ),
      ),
    )..onChangePassword = () => throw ApiException.invalidPassword();
    await _pump(tester, notifier: notifier);

    await tester.enterText(
      find.byKey(const Key('forceChange.current')),
      'badpass',
    );
    await tester.enterText(
      find.byKey(const Key('forceChange.next')),
      'longenough',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('forceChange.submit')));
    await tester.pumpAndSettle();

    expect(find.text('目前密碼不正確'), findsOneWidget);
  });

  testWidgets('successful submit invokes notifier.changePassword',
      (tester) async {
    var called = false;
    final notifier = FakeAuthNotifier(
      AsyncValue<AuthState>.data(
        AuthState.authenticated(
          user: _user,
          org: _org,
          needsPasswordChange: true,
        ),
      ),
    )..onChangePassword = () async {
        called = true;
      };
    await _pump(tester, notifier: notifier);

    await tester.enterText(
      find.byKey(const Key('forceChange.current')),
      'old1234',
    );
    await tester.enterText(
      find.byKey(const Key('forceChange.next')),
      'new12345',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('forceChange.submit')));
    await tester.pumpAndSettle();

    expect(called, isTrue);
  });
}

bool _isEnabled(WidgetTester tester, Finder f) {
  final w = tester.widget<FilledButton>(f);
  return w.onPressed != null;
}

Future<void> _pump(
  WidgetTester tester, {
  required FakeAuthNotifier notifier,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        authProvider.overrideWith(() => notifier),
      ],
      child: const MaterialApp(
        locale: Locale('zh', 'TW'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: <LocalizationsDelegate<Object>>[
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        home: ForcePasswordChangeScreen(),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

const _user = AppUser(
  id: 'u1',
  username: 'alice',
  displayName: 'Alice',
  status: AppUserStatus.active,
  needsPasswordChange: true,
  createdAt: '2025-01-01T00:00:00Z',
);

const _org = Org(
  id: 'o1',
  name: 'Acme',
  code: 'ABCDEFGHIJ',
  ownerId: 'u1',
  timezone: 'Asia/Taipei',
  checkin: OrgCheckin(transferEnabled: true),
);
