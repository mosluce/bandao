import 'package:flutter/material.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../core/api/models/checkin_status.dart';
import '../features/checkin/data/location_ping_processor.dart';
import '../features/checkin/data/queue_processor.dart';
import '../features/checkin/state/checkin_status_provider.dart';
import '../features/checkin/state/location_tracking_controller.dart';
import '../l10n/app_localizations.dart';
import '../shared/theme/app_theme.dart';
import 'router.dart';

/// Root widget. The locale is hard-coded to zh-TW for v1; ARB infrastructure
/// is in place when more locales arrive.
class ArgusApp extends ConsumerWidget {
  const ArgusApp({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    // Start the foreground queue runner. Kept-alive so it lives for the
    // whole app session. Tests that pump MaterialApp.router directly (rather
    // than ArgusApp) avoid spinning up the timer.
    ref.watch(queueProcessorRunnerProvider);
    // Same shape for the location-pings batch processor.
    ref.watch(locationPingProcessorRunnerProvider);
    // Boots the start/stop decider for the location tracker; subscribes to
    // checkinStatusProvider + effectiveStatusProvider internally.
    ref.watch(locationTrackingControllerProvider);

    // When the server-confirmed status transitions to off_duty (clock_out
    // landed), tell the location-pings processor to drain whatever's left
    // — bypasses the count / time thresholds.
    ref.listen<AsyncValue<CheckinUserStatusDto?>>(
      checkinStatusProvider,
      (prev, next) {
        final prevStatus = prev?.valueOrNull?.status;
        final nextStatus = next.valueOrNull?.status;
        if (prevStatus != null &&
            prevStatus != AppUserCheckinStatus.offDuty &&
            nextStatus == AppUserCheckinStatus.offDuty) {
          final processor = ref.read(locationPingProcessorProvider);
          processor.requestFinalFlush();
          processor.tick();
        }
      },
    );

    final router = ref.watch(routerProvider);
    return MaterialApp.router(
      title: 'Argus',
      theme: AppTheme.light(),
      debugShowCheckedModeBanner: false,
      locale: const Locale('zh', 'TW'),
      supportedLocales: AppLocalizations.supportedLocales,
      localizationsDelegates: const <LocalizationsDelegate<Object>>[
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      routerConfig: router,
    );
  }
}
