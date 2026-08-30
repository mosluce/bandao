import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';

import 'package:bandao_app/core/env/env.dart';
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

  group('override visibility', () {
    testWidgets('inputs start empty when no override is stored',
        (tester) async {
      await _pump(tester, FakeSecureStorage());

      expect(_textOf(tester, 'server_config.url'), '');
      expect(_textOf(tester, 'server_config.privacy_url'), '');
    });

    testWidgets('inputs show the stored override when one exists',
        (tester) async {
      final storage = FakeSecureStorage(
        apiBaseUrlOverride: 'https://api.myco.com',
      );
      await storage.writePrivacyUrlOverride('https://myco.com/privacy');
      await _pump(tester, storage);

      expect(_textOf(tester, 'server_config.url'), 'https://api.myco.com');
      expect(
        _textOf(tester, 'server_config.privacy_url'),
        'https://myco.com/privacy',
      );
    });
  });

  group('accidental pinning', () {
    testWidgets('opening and saving without editing writes no override',
        (tester) async {
      // The regression this change exists to prevent. The inputs used to be
      // pre-filled with Env.compileTimeDefault(), so this exact sequence — open
      // the screen, tap 儲存, change nothing — froze that build's default into
      // secure storage, where it outranked the compile-time value forever.
      final storage = FakeSecureStorage();
      await _pump(tester, storage);

      await tester.tap(find.byKey(const Key('server_config.save')));
      await tester.pumpAndSettle();

      expect(await storage.readApiBaseUrlOverride(), isNull);
    });

    testWidgets('same for the privacy section', (tester) async {
      final storage = FakeSecureStorage();
      await _pump(tester, storage);

      await tester.tap(find.byKey(const Key('server_config.privacy_save')));
      await tester.pumpAndSettle();

      expect(await storage.readPrivacyUrlOverride(), isNull);
    });
  });

  group('blank means clear', () {
    testWidgets('blank save removes an existing base-URL override',
        (tester) async {
      final storage = FakeSecureStorage(
        apiBaseUrlOverride: 'https://api.myco.com',
      );
      await _pump(tester, storage);

      await tester.enterText(find.byKey(const Key('server_config.url')), '');
      await tester.tap(find.byKey(const Key('server_config.save')));
      await tester.pumpAndSettle();

      expect(await storage.readApiBaseUrlOverride(), isNull);
    });

    testWidgets('blank save removes an existing privacy override',
        (tester) async {
      final storage = FakeSecureStorage();
      await storage.writePrivacyUrlOverride('https://myco.com/privacy');
      await _pump(tester, storage);

      await tester.enterText(
        find.byKey(const Key('server_config.privacy_url')),
        '',
      );
      await tester.tap(find.byKey(const Key('server_config.privacy_save')));
      await tester.pumpAndSettle();

      expect(await storage.readPrivacyUrlOverride(), isNull);
    });
  });

  testWidgets('a typed value equal to the compile-time default is still stored',
      (tester) async {
    // The default is not overridable and nothing here redefines it — what the
    // override sets is the CURRENT value. So a URL the user deliberately typed
    // is persisted as typed, even when it happens to equal the default. The
    // screen does not second-guess the input; the empty seeding above is what
    // prevents accidental pinning.
    final storage = FakeSecureStorage();
    await _pump(tester, storage);

    final theDefault = Env.compileTimeDefault();
    await tester.enterText(
      find.byKey(const Key('server_config.url')),
      theDefault,
    );
    await tester.tap(find.byKey(const Key('server_config.save')));
    await tester.pumpAndSettle();

    expect(await storage.readApiBaseUrlOverride(), theDefault);
  });

  testWidgets('a malformed privacy URL is rejected and not persisted',
      (tester) async {
    // Release-mode rules (https required) are exercised against
    // validateBaseUrlOverride in api_base_url_test.dart; the screen now
    // delegates to that same function instead of its own inline check, so
    // this asserts the delegation holds for the malformed case, which is the
    // half observable in a debug-mode widget test.
    final storage = FakeSecureStorage();
    await _pump(tester, storage);

    await tester.enterText(
      find.byKey(const Key('server_config.privacy_url')),
      'not a url',
    );
    await tester.tap(find.byKey(const Key('server_config.privacy_save')));
    await tester.pumpAndSettle();

    expect(await storage.readPrivacyUrlOverride(), isNull);
  });
}

String _textOf(WidgetTester tester, String key) =>
    tester.widget<TextField>(find.byKey(Key(key))).controller!.text;

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
