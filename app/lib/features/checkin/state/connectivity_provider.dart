import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

bool _isOnline(List<ConnectivityResult> results) {
  if (results.isEmpty) return false;
  return results.any((r) => r != ConnectivityResult.none);
}

/// Emits `true` when the device has any non-`none` connectivity, `false`
/// otherwise. The processor uses this to skip ticks while offline (so the
/// backoff window doesn't grow during a long offline stretch).
final connectivityProvider = StreamProvider<bool>((ref) async* {
  final c = Connectivity();
  yield _isOnline(await c.checkConnectivity());
  yield* c.onConnectivityChanged.map(_isOnline);
});
