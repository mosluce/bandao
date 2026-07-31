import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:geolocator/geolocator.dart';

import 'package:bandao_app/core/api/models/app_user.dart';
import 'package:bandao_app/core/api/models/checkin_status.dart';
import 'package:bandao_app/core/api/models/org.dart';
import 'package:bandao_app/features/auth/state/auth_provider.dart';
import 'package:bandao_app/features/auth/state/auth_state.dart';
import 'package:bandao_app/features/checkin/presentation/home_buttons.dart';
import 'package:bandao_app/features/checkin/state/checkin_label_provider.dart';
import 'package:bandao_app/features/checkin/state/effective_status_provider.dart';
import 'package:bandao_app/features/checkin/state/location_permission_provider.dart';
import 'package:bandao_app/l10n/app_localizations.dart';

import '../../../helpers/fake_auth_notifier.dart';

void main() {
  group('HomeButtons by status', () {
    testWidgets('off_duty shows only [上班]', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.offDuty);
      expect(find.text('上班'), findsOneWidget);
      expect(find.text('下班'), findsNothing);
      expect(find.text('轉出'), findsNothing);
      expect(find.text('轉入'), findsNothing);
    });

    testWidgets('on_site shows [下班] and [轉出]', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.onSite);
      expect(find.text('下班'), findsOneWidget);
      expect(find.text('轉出'), findsOneWidget);
      expect(find.text('上班'), findsNothing);
      expect(find.text('轉入'), findsNothing);
    });

    testWidgets('in_transit shows [下班] and [轉入]', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.inTransit);
      expect(find.text('下班'), findsOneWidget);
      expect(find.text('轉入'), findsOneWidget);
      expect(find.text('上班'), findsNothing);
      expect(find.text('轉出'), findsNothing);
    });
  });

  group('HomeButtons gating', () {
    testWidgets('buttons disabled when permission deniedForever',
        (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.offDuty,
        permission: LocationPermission.deniedForever,
      );
      final button = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(button.onPressed, isNull);
    });

    testWidgets('buttons enabled when permission denied (never-asked)',
        (tester) async {
      // On iOS, `LocationPermission.denied` covers the never-asked case —
      // the button stays enabled so first-tap can fire the system prompt.
      await _pump(
        tester,
        status: AppUserCheckinStatus.offDuty,
        permission: LocationPermission.denied,
      );
      final button = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(button.onPressed, isNotNull);
    });

    testWidgets('buttons enabled when permission granted', (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.offDuty,
        permission: LocationPermission.whileInUse,
      );
      final button = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(button.onPressed, isNotNull);
    });
  });

  group('HomeButtons honors transferEnabled', () {
    testWidgets('on_site collapses to [下班] only when transfers disabled',
        (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.onSite,
        transferEnabled: false,
      );
      expect(find.text('下班'), findsOneWidget);
      expect(find.text('轉出'), findsNothing);
      expect(find.text('上班'), findsNothing);
      expect(find.text('轉入'), findsNothing);
    });

    testWidgets('in_transit collapses to [下班] only when transfers disabled',
        (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.inTransit,
        transferEnabled: false,
      );
      expect(find.text('下班'), findsOneWidget);
      expect(find.text('轉入'), findsNothing);
      expect(find.text('上班'), findsNothing);
      expect(find.text('轉出'), findsNothing);
    });

    testWidgets('off_duty unaffected by transferEnabled', (tester) async {
      await _pump(
        tester,
        status: AppUserCheckinStatus.offDuty,
        transferEnabled: false,
      );
      expect(find.text('上班'), findsOneWidget);
      expect(find.text('下班'), findsNothing);
    });
  });
  group('HomeButtons label gate', () {
    // The label is collected BEFORE the press — that is what keeps the
    // tap→enqueue path free of any dialog. The buttons are the gate.
    testWidgets('disabled while the label is empty', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.offDuty, label: '');

      expect(find.text('上班'), findsOneWidget);
      expect(_enabled(tester, '上班'), isFalse);
    });

    testWidgets('enabled once a label is supplied', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.offDuty, label: '乙工地');

      expect(_enabled(tester, '上班'), isTrue);
    });

    testWidgets('a whitespace-only label does not count', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.offDuty, label: '   ');

      expect(_enabled(tester, '上班'), isFalse);
    });

    testWidgets('an over-long label does not count', (tester) async {
      // Server validates 1–120 characters; the device must agree so the
      // failure happens here rather than at submit time.
      await _pump(
        tester,
        status: AppUserCheckinStatus.offDuty,
        label: 'x' * 121,
      );

      expect(_enabled(tester, '上班'), isFalse);
    });

    testWidgets('no dialog is shown when the label is empty', (tester) async {
      await _pump(tester, status: AppUserCheckinStatus.offDuty, label: '');

      expect(find.byType(Dialog), findsNothing);
      expect(find.byType(BottomSheet), findsNothing);
    });
  });
}

/// Whether the button carrying [text] is tappable.
bool _enabled(WidgetTester tester, String text) {
  final button = tester.widget<ButtonStyleButton>(
    find.ancestor(
      of: find.text(text),
      matching: find.byWidgetPredicate((w) => w is ButtonStyleButton),
    ),
  );
  return button.onPressed != null;
}

Future<void> _pump(
  WidgetTester tester, {
  required AppUserCheckinStatus status,
  LocationPermission permission = LocationPermission.whileInUse,
  bool transferEnabled = true,
  // The buttons now gate on a label as well as on permission. Default to a
  // filled one so the pre-existing tests keep asserting what they were
  // written to assert; the label gate has its own group below.
  String label = '甲工地',
}) async {
  final auth = AuthState.authenticated(
    user: const AppUser(
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
      checkin: OrgCheckin(transferEnabled: transferEnabled),
    ),
    needsPasswordChange: false,
  );

  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        checkinLabelProvider.overrideWith((ref) => label),
        effectiveStatusProvider.overrideWithValue(EffectiveStatus(
          status: status,
          hasPendingTransition: false,
        ),),
        locationPermissionProvider.overrideWith(
          () => _FixedPermissionNotifier(permission),
        ),
        authProvider.overrideWith(
          () => FakeAuthNotifier(AsyncValue.data(auth)),
        ),
      ],
      child: MaterialApp(
        home: const Scaffold(body: HomeButtons()),
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

class _FixedPermissionNotifier extends LocationPermissionNotifier {
  _FixedPermissionNotifier(this.fixed);
  final LocationPermission fixed;

  @override
  Future<LocationPermission> build() async => fixed;
}
