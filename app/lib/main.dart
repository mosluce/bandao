import 'dart:ui';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_crashlytics/firebase_crashlytics.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app/bandao_app.dart';
import 'features/checkin/data/background_sync.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // Firebase + Crashlytics. Initialize before any plugin that may throw,
  // so uncaught errors during boot are reported. We deliberately do NOT
  // call FirebaseCrashlytics.instance.setUserIdentifier — crashes stay
  // unlinked from Bandao identity per the mobile-release spec.
  await Firebase.initializeApp();
  FlutterError.onError = FirebaseCrashlytics.instance.recordFlutterFatalError;
  PlatformDispatcher.instance.onError = (Object error, StackTrace stack) {
    FirebaseCrashlytics.instance.recordError(error, stack, fatal: true);
    return true;
  };
  await initBackgroundSync();
  // One-shot registration so the OS knows the BGProcessingTask identifier
  // exists and may schedule it. Subsequent calls (from enqueue) keep the
  // existing task per `ExistingWorkPolicy.keep`.
  await requestBackgroundDrain();
  runApp(const ProviderScope(child: BandaoApp()));
}
