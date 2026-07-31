import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Maximum label length, mirroring the server's `manual_label` validation so
/// an over-long value fails on the device rather than at submit time.
const int checkinLabelMaxLength = 120;

/// The label that will be attached to the next check-in.
///
/// Deliberately NOT sticky across events: `enqueueEvent` clears it after each
/// successful enqueue. A worker who moves to another site and forgets to
/// update a retained value would otherwise produce confidently wrong records
/// with nothing in the UI to signal it — and 16.7% of days in the imported
/// history visit more than one site.
final checkinLabelProvider = StateProvider<String>((ref) => '');

/// Whether the current label satisfies the server's 1–120 character rule.
///
/// Counts runes, not UTF-16 code units, because the server counts Unicode
/// scalar values (`chars().count()`). The two agree for CJK but not for
/// anything outside the BMP, and disagreeing would mean the device accepts a
/// label the server then rejects.
final checkinLabelIsValidProvider = Provider<bool>((ref) {
  final v = ref.watch(checkinLabelProvider).trim();
  return v.isNotEmpty && v.runes.length <= checkinLabelMaxLength;
});
