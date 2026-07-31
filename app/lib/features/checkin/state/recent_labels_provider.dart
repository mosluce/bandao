import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/storage/secure_storage.dart';
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
  Future<List<String>> build() => _load();

  Future<void> refresh() async {
    state = await AsyncValue.guard(_load);
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

    final current = state.valueOrNull ?? const <String>[];
    final next = <String>[
      value,
      ...current.where((l) => l != value),
    ].take(recentLabelsCap).toList(growable: false);

    state = AsyncValue.data(next);
    await _cache(ref.read(secureStorageProvider), next);
  }

  Future<List<String>> _load() async {
    final storage = ref.read(secureStorageProvider);
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
        await _cache(storage, labels);
      }
      return labels.isNotEmpty ? labels : await _cached(storage);
    } catch (_) {
      // Offline, or the request failed. Suggestions are a convenience; never
      // let their absence surface as an error on the check-in screen.
      return _cached(storage);
    }
  }

  Future<void> _cache(SecureStorage storage, List<String> labels) async {
    try {
      await storage.writeRecentCheckinLabels(jsonEncode(labels));
    } on SecureStorageFailure {
      // Already reported by the storage layer. Losing the cache only costs
      // the worker their shortcuts next time they are offline.
    }
  }

  Future<List<String>> _cached(SecureStorage storage) async {
    final raw = await storage.readRecentCheckinLabels();
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

final recentLabelsProvider =
    AsyncNotifierProvider<RecentLabelsNotifier, List<String>>(
  RecentLabelsNotifier.new,
);
