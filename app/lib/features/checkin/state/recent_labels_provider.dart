import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/storage/secure_storage.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../data/checkin_repository.dart';
import '../data/recent_labels.dart';

/// Suggestions for the check-in label field.
///
/// Fetches the AppUser's own recent events — the endpoint already returns
/// `location.manual_label`, so no new API surface is involved — computes the
/// most-used labels, and caches the result.
///
/// The cache is what makes this usable offline, which matters because
/// offline check-in is a core scenario of this app: the label is required on
/// every event, so a worker out of signal with no suggestions would be left
/// typing. On any fetch failure the cached list is served instead.
class RecentLabelsNotifier extends AsyncNotifier<List<String>> {
  @override
  Future<List<String>> build() {
    // Watch the resolved AppUser id, not the whole AsyncValue. Devices are
    // shared between workers here — it is why `wipeForOtherUsers` exists for
    // the event queue — so the suggestions must rebuild when the signed-in
    // AppUser changes; otherwise the in-memory list survives a logout and
    // shows one worker's site names, which are customer names, to the next.
    //
    // Watching `authProvider` itself would make this provider wait on auth's
    // loading state and never resolve during bootstrap.
    final userId = ref.watch(_signedInAppUserIdProvider);
    return _load(userId);
  }

  Future<void> refresh() async {
    final userId = ref.read(_signedInAppUserIdProvider);
    state = await AsyncValue.guard(() => _load(userId));
  }

  /// Record a label the worker just used, without waiting for the server.
  ///
  /// Refetching after an enqueue would not work: the event sits in the local
  /// queue until it syncs, so the server does not know about it yet — and
  /// offline it never would. The suggestion has to come from the device.
  ///
  /// Inserted at the front rather than appended. Frequency ordering is
  /// restored by the next successful fetch; until then, the label just used
  /// is overwhelmingly the one wanted next, because 83.3% of days in the
  /// imported history use a single label from clock-in to clock-out.
  Future<void> remember(String label) async {
    final value = label.trim();
    if (value.isEmpty) return;
    final userId = ref.read(_signedInAppUserIdProvider);
    if (userId == null) return;

    final current = state.valueOrNull ?? const <String>[];
    final next = <String>[
      value,
      ...current.where((l) => l != value),
    ].take(recentLabelsCap).toList(growable: false);

    state = AsyncValue.data(next);
    await _cache(ref.read(secureStorageProvider), userId, next);
  }

  Future<List<String>> _load(String? userId) async {
    final storage = ref.read(secureStorageProvider);
    // No session: offer nothing rather than whatever the last user left.
    if (userId == null) return const <String>[];
    try {
      final repo = await ref.read(checkinRepositoryProvider.future);
      final now = DateTime.now();
      final events = await repo.events(
        limit: 200,
        from: now.subtract(recentLabelsWindow),
        to: now,
      );
      final labels = computeRecentLabels(events, now: now);
      // Only overwrite the cache on a successful fetch — a fetch that
      // returned nothing because the window is empty must not wipe a list
      // the worker was relying on.
      if (labels.isNotEmpty) {
        await _cache(storage, userId, labels);
      }
      return labels.isNotEmpty ? labels : await _cached(storage, userId);
    } catch (_) {
      // Offline, or the request failed. Suggestions are a convenience; never
      // let their absence surface as an error on the check-in screen.
      return _cached(storage, userId);
    }
  }

  Future<void> _cache(
    SecureStorage storage,
    String appUserId,
    List<String> labels,
  ) async {
    try {
      await storage.writeRecentCheckinLabels(appUserId, jsonEncode(labels));
    } on SecureStorageFailure {
      // Already reported by the storage layer. Losing the cache only costs
      // the worker their shortcuts next time they are offline.
    }
  }

  Future<List<String>> _cached(SecureStorage storage, String appUserId) async {
    final raw = await storage.readRecentCheckinLabels(appUserId);
    if (raw == null || raw.isEmpty) return const <String>[];
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! List) return const <String>[];
      return decoded.whereType<String>().toList(growable: false);
    } catch (_) {
      return const <String>[];
    }
  }
}

/// The signed-in AppUser's id, or null. Kept as its own provider so
/// `RecentLabelsNotifier` can depend on the *identity* rather than on auth's
/// async state — the latter would stall this provider during bootstrap.
final _signedInAppUserIdProvider = Provider<String?>((ref) {
  final auth = ref.watch(authProvider).valueOrNull;
  return auth is AuthAuthenticated ? auth.user.id : null;
});

final recentLabelsProvider =
    AsyncNotifierProvider<RecentLabelsNotifier, List<String>>(
  RecentLabelsNotifier.new,
);
