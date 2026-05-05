import 'package:flutter_riverpod/flutter_riverpod.dart';

/// One-shot toast carrier for "前個帳號的 N 筆未送事件已清除". Set by the
/// auth notifier after the queue wipe; cleared by the listener after the
/// SnackBar is shown.
final pendingHandoverNoticeProvider = StateProvider<String?>((ref) => null);
