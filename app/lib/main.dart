import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'app/bandao_app.dart';
import 'features/checkin/data/background_sync.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await initBackgroundSync();
  // One-shot registration so the OS knows the BGProcessingTask identifier
  // exists and may schedule it. Subsequent calls (from enqueue) keep the
  // existing task per `ExistingWorkPolicy.keep`.
  await requestBackgroundDrain();
  runApp(const ProviderScope(child: BandaoApp()));
}
