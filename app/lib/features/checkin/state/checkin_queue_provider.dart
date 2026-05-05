import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../data/checkin_queue_db.dart';

/// Stream of queue rows for the currently authenticated AppUser. Emits an
/// empty list when no user is logged in.
final checkinQueueProvider = StreamProvider<List<QueueRow>>((ref) {
  final db = ref.watch(checkinQueueDbProvider);
  final auth = ref.watch(authProvider).valueOrNull;
  if (auth is! AuthAuthenticated) {
    return Stream<List<QueueRow>>.value(const <QueueRow>[]);
  }
  return db.watchAll(auth.user.id);
});
