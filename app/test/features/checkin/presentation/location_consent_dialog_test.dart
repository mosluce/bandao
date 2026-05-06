import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:plugin_platform_interface/plugin_platform_interface.dart';
import 'package:url_launcher_platform_interface/link.dart';
import 'package:url_launcher_platform_interface/url_launcher_platform_interface.dart';

import 'package:bandao_app/core/storage/privacy_url.dart';
import 'package:bandao_app/features/checkin/presentation/location_consent_dialog.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late _FakeUrlLauncher fakeLauncher;
  setUp(() {
    fakeLauncher = _FakeUrlLauncher();
    UrlLauncherPlatform.instance = fakeLauncher;
  });

  group('showLocationConsentDialog', () {
    testWidgets('renders all bullets + privacy link + buttons in zh',
        (tester) async {
      Future<bool>? future;
      await _pumpHost(
        tester,
        onPressed: (BuildContext ctx, WidgetRef ref) {
          future = showLocationConsentDialog(ctx, ref);
        },
      );
      await tester.tap(find.byType(ElevatedButton));
      await tester.pumpAndSettle();

      expect(find.text('啟用定位追蹤'), findsOneWidget);
      expect(find.textContaining('上班期間約每分鐘記錄'), findsOneWidget);
      expect(find.textContaining('移動超過 100 公尺'), findsOneWidget);
      expect(find.textContaining('保存 90 天後自動清除'), findsOneWidget);
      expect(find.textContaining('管理員查閱'), findsOneWidget);
      expect(find.text('查看完整隱私政策'), findsOneWidget);
      expect(find.text('取消'), findsOneWidget);
      expect(find.text('同意並上班'), findsOneWidget);

      // Dismiss to clean up the awaited future.
      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      await future;
    });

    testWidgets('cancel returns false', (tester) async {
      late Future<bool> future;
      await _pumpHost(
        tester,
        onPressed: (BuildContext ctx, WidgetRef ref) {
          future = showLocationConsentDialog(ctx, ref);
        },
      );
      await tester.tap(find.byType(ElevatedButton));
      await tester.pumpAndSettle();

      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      expect(await future, isFalse);
    });

    testWidgets('confirm returns true', (tester) async {
      late Future<bool> future;
      await _pumpHost(
        tester,
        onPressed: (BuildContext ctx, WidgetRef ref) {
          future = showLocationConsentDialog(ctx, ref);
        },
      );
      await tester.tap(find.byType(ElevatedButton));
      await tester.pumpAndSettle();

      await tester.tap(find.text('同意並上班'));
      await tester.pumpAndSettle();
      expect(await future, isTrue);
    });

    testWidgets('privacy link tap invokes url_launcher', (tester) async {
      late Future<bool> future;
      await _pumpHost(
        tester,
        onPressed: (BuildContext ctx, WidgetRef ref) {
          future = showLocationConsentDialog(ctx, ref);
        },
      );
      await tester.tap(find.byType(ElevatedButton));
      await tester.pumpAndSettle();

      await tester.tap(find.text('查看完整隱私政策'));
      await tester.pumpAndSettle();
      expect(fakeLauncher.launchedUrls, contains('https://example.com/privacy'));

      await tester.tap(find.text('取消'));
      await tester.pumpAndSettle();
      await future;
    });
  });
}

class _FakeUrlLauncher extends UrlLauncherPlatform
    with MockPlatformInterfaceMixin {
  final List<String> launchedUrls = <String>[];

  @override
  LinkDelegate? get linkDelegate => null;

  @override
  Future<bool> canLaunch(String url) async => true;

  @override
  Future<bool> launch(
    String url, {
    required bool useSafariVC,
    required bool useWebView,
    required bool enableJavaScript,
    required bool enableDomStorage,
    required bool universalLinksOnly,
    required Map<String, String> headers,
    String? webOnlyWindowName,
  }) async {
    launchedUrls.add(url);
    return true;
  }

  @override
  Future<bool> launchUrl(String url, LaunchOptions options) async {
    launchedUrls.add(url);
    return true;
  }

  @override
  Future<bool> closeWebView() async => true;

  @override
  Future<bool> supportsMode(PreferredLaunchMode mode) async => true;

  @override
  Future<bool> supportsCloseForMode(PreferredLaunchMode mode) async => true;
}

Future<void> _pumpHost(
  WidgetTester tester, {
  required void Function(BuildContext context, WidgetRef ref) onPressed,
}) async {
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        effectivePrivacyUrlProvider
            .overrideWith((ref) async => 'https://example.com/privacy'),
      ],
      child: MaterialApp(
        home: Scaffold(
          body: Consumer(
            builder: (context, ref, _) => Center(
              child: ElevatedButton(
                onPressed: () => onPressed(context, ref),
                child: const Text('open'),
              ),
            ),
          ),
        ),
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
