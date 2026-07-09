import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:bandao_app/core/storage/secure_storage.dart';
import 'package:bandao_app/features/auth/presentation/login_screen.dart';
import 'package:bandao_app/features/auth/state/auth_provider.dart';
import 'package:bandao_app/features/auth/state/auth_state.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

import '../../../helpers/fake_auth_notifier.dart';
import '../../../helpers/fake_secure_storage.dart';

void main() {
  testWidgets('shows the official-default connection when no override is set',
      (tester) async {
    await _pump(tester, FakeSecureStorage());

    expect(find.text('目前連線：官方預設'), findsOneWidget);
    expect(find.byKey(const Key('login.server_config')), findsOneWidget);
  });

  testWidgets('shows the custom host when an override is set', (tester) async {
    await _pump(
      tester,
      FakeSecureStorage(apiBaseUrlOverride: 'https://api.myco.com'),
    );

    expect(find.text('目前連線：自訂 api.myco.com'), findsOneWidget);
  });
}

Future<void> _pump(WidgetTester tester, SecureStorage storage) async {
  final notifier = FakeAuthNotifier(
    const AsyncValue<AuthState>.data(AuthState.unauthenticated()),
  );
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        authProvider.overrideWith(() => notifier),
        secureStorageProvider.overrideWithValue(storage),
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
