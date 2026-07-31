import '../../../core/api/models/checkin_event.dart';

/// How far back the suggestions look.
const Duration recentLabelsWindow = Duration(days: 30);

/// How many suggestions are offered.
///
/// The median AppUser has used 7 distinct labels in total, so 6 covers nearly
/// everything without turning the row into a scrolling list.
const int recentLabelsCap = 6;

/// The AppUser's own labels, most-used first.
///
/// Ordered by frequency rather than recency deliberately: the dominant site
/// then keeps a stable position and becomes muscle memory, which is the whole
/// point of offering suggestions for a field that has to be filled on every
/// single check-in.
///
/// Events with no label are skipped — every event the app itself has created
/// so far has `manual_label: null`, so on a freshly-migrated Org the imported
/// history is the only source there is.
List<String> computeRecentLabels(
  List<CheckinEventDto> events, {
  required DateTime now,
  int cap = recentLabelsCap,
  Duration window = recentLabelsWindow,
}) {
  final cutoff = now.subtract(window);
  final counts = <String, int>{};
  // Preserves first-seen order so ties break toward the label the user
  // encountered most recently in the (newest-first) feed rather than
  // arbitrarily.
  for (final e in events) {
    final label = e.location.manualLabel?.trim();
    if (label == null || label.isEmpty) continue;
    final at = DateTime.tryParse(e.occurredAtClient);
    if (at == null || at.isBefore(cutoff)) continue;
    counts[label] = (counts[label] ?? 0) + 1;
  }

  final ordered = counts.keys.toList();
  final order = {for (var i = 0; i < ordered.length; i++) ordered[i]: i};
  ordered.sort((a, b) {
    final byCount = counts[b]!.compareTo(counts[a]!);
    if (byCount != 0) return byCount;
    return order[a]!.compareTo(order[b]!);
  });

  return ordered.take(cap).toList(growable: false);
}
