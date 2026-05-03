import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:argus_app/core/api/api_error.dart';
import 'package:argus_app/core/storage/secure_storage.dart';
import 'package:argus_app/features/auth/presentation/login_screen.dart';
import 'package:argus_app/features/auth/state/auth_provider.dart';
import 'package:argus_app/features/auth/state/auth_state.dart';
import 'package:argus_app/l10n/app_localizations.dart';

import '../../../helpers/fake_auth_notifier.dart';
import '../../../helpers/fake_secure_storage.dart';

void main() {
  testWidgets('submit disabled until all three fields filled',
      (tester) async {
    final notifier = FakeAuthNotifier(
      const AsyncValue<AuthState>.data(AuthState.unauthenticated()),
    );
    await _pump(tester, notifier: notifier);

    final submit = find.byKey(const Key('login.submit'));
    expect(_isEnabled(tester, submit), isFalse);

    await tester.enterText(find.byKey(const Key('login.org_code')), 'ORG');
    await tester.pump();
    expect(_isEnabled(tester, submit), isFalse);

    await tester.enterText(find.byKey(const Key('login.username')), 'alice');
    await tester.pump();
    expect(_isEnabled(tester, submit), isFalse);

    await tester.enterText(find.byKey(const Key('login.password')), 'pwd');
    await tester.pump();
    expect(_isEnabled(tester, submit), isTrue);
  });

  testWidgets('INVALID_CREDENTIALS renders friendly Chinese',
      (tester) async {
    final notifier = FakeAuthNotifier(
      const AsyncValue<AuthState>.data(AuthState.unauthenticated()),
    )..onLogin = () => throw ApiException.invalidCredentials();
    await _pump(tester, notifier: notifier);

    await tester.enterText(find.byKey(const Key('login.org_code')), 'ORG');
    await tester.enterText(find.byKey(const Key('login.username')), 'alice');
    await tester.enterText(find.byKey(const Key('login.password')), 'wrong');
    await tester.pump();
    await tester.tap(find.byKey(const Key('login.submit')));
    await tester.pumpAndSettle();

    expect(find.text('帳號、密碼或組織代碼錯誤'), findsOneWidget);
  });

  testWidgets('successful submit calls notifier.login', (tester) async {
    var loginCalled = false;
    final notifier = FakeAuthNotifier(
      const AsyncValue<AuthState>.data(AuthState.unauthenticated()),
    )..onLogin = () async {
        loginCalled = true;
      };
    await _pump(tester, notifier: notifier);

    await tester.enterText(find.byKey(const Key('login.org_code')), 'ORG');
    await tester.enterText(find.byKey(const Key('login.username')), 'alice');
    await tester.enterText(find.byKey(const Key('login.password')), 'pass1234');
    await tester.pump();
    await tester.tap(find.byKey(const Key('login.submit')));
    await tester.pumpAndSettle();

    expect(loginCalled, isTrue);
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
        secureStorageProvider.overrideWithValue(FakeSecureStorage()),
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
        home: LoginScreen(),
      ),
    ),
  );
  await tester.pumpAndSettle();
}
