import 'package:flutter_riverpod/flutter_riverpod.dart';

/// Raised when login succeeded but the bearer token could not be persisted:
/// the session is valid for this process but will not survive a cold start.
///
/// Carries a flag rather than a message so the wording is resolved where a
/// `BuildContext` exists and can therefore be localized. The neighbouring
/// `pendingHandoverNoticeProvider` carries a pre-built Chinese string, which
/// is why English builds see Chinese for that one — not a pattern to copy.
///
/// The notifier sets it; the home screen shows it once and clears it. Kept
/// separate from the handover channel so neither notice can overwrite the
/// other when both fire on the same login.
final pendingSessionNotPersistedProvider = StateProvider<bool>((ref) => false);
