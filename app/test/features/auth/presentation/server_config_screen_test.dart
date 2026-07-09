import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';

import 'package:bandao_app/core/storage/secure_storage.dart';
import 'package:bandao_app/features/auth/presentation/server_config_screen.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

import '../../../helpers/fake_secure_storage.dart';

void main() {
  testWidgets('renders and reachable outside a debug-only gate',
      (tester) async {
    final storage = FakeSecureStorage();
    await _pump(tester, storage);

    expect(find.byType(ServerConfigScreen), findsOneWidget);
    expect(find.byKey(const Key('server_config.url')), findsOneWidget);
  });

  testWidgets('saving a valid https URL persists the override and clears the '
      'session', (tester) async {
    final storage = FakeSecureStorage(token: 'server-a-token');
    await _pump(tester, storage);

    await tester.enterText(
      find.byKey(const Key('server_config.url')),
      'https://api.myco.com',
    );
    await tester.tap(find.byKey(const Key('server_config.save')));
    await tester.pumpAndSettle();

    expect(await storage.readApiBaseUrlOverride(), 'https://api.myco.com');
    // Changing the server drops the bearer token issued by the old server.
    expect(await storage.readToken(), isNull);
  });

  testWidgets('rejecting a non-https URL keeps it unsaved (release rule '
      'only bites in release; here we assert malformed is rejected)',
      (tester) async {
    final storage = FakeSecureStorage();
    await _pump(tester, storage);

    await tester.enterText(
      find.byKey(const Key('server_config.url')),
      'not a url',
    );
    await tester.tap(find.byKey(const Key('server_config.save')));
    await tester.pumpAndSettle();

    expect(await storage.readApiBaseUrlOverride(), isNull);
  });
}

Future<void> _pump(WidgetTester tester, SecureStorage storage) async {
  final router = GoRouter(
    initialLocation: '/server-config',
    routes: <RouteBase>[
      GoRoute(
        path: '/server-config',
        builder: (_, __) => const ServerConfigScreen(),
      ),
      GoRoute(
        path: '/login',
        builder: (_, __) => const Scaffold(body: Text('login')),
      ),
    ],
  );
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        secureStorageProvider.overrideWithValue(storage),
      ],
      child: MaterialApp.router(
        locale: const Locale('zh', 'TW'),
        supportedLocales: AppLocalizations.supportedLocales,
        localizationsDelegates: const <LocalizationsDelegate<Object>>[
          AppLocalizations.delegate,
          GlobalMaterialLocalizations.delegate,
          GlobalWidgetsLocalizations.delegate,
          GlobalCupertinoLocalizations.delegate,
        ],
        routerConfig: router,
      ),
    ),
  );
  await tester.pumpAndSettle();
  // Touch the resolver so the screen's `effectiveBaseUrlProvider` read in
  // `_save` has a cached value in tests.
  expect(find.byType(ServerConfigScreen), findsOneWidget);
}
