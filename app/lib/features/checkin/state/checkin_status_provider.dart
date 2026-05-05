import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/api/models/checkin_status.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../data/checkin_repository.dart';

/// Server-confirmed checkin status for the current AppUser. Refetches when
/// the auth state reaches `authenticated(needsPasswordChange:false)`.
class CheckinStatusNotifier extends AsyncNotifier<CheckinUserStatusDto?> {
  @override
  Future<CheckinUserStatusDto?> build() async {
    final auth = await ref.watch(authProvider.future);
    if (auth is! AuthAuthenticated || auth.needsPasswordChange) {
      return null;
    }
    return _fetch();
  }

  Future<CheckinUserStatusDto?> _fetch() async {
    final repo = await ref.read(checkinRepositoryProvider.future);
    return repo.status();
  }

  /// Force a re-fetch — invoked after a successful submit so the status pill
  /// reflects the new server-confirmed state.
  Future<void> refresh() async {
    state = const AsyncValue.loading();
    state = await AsyncValue.guard(_fetch);
  }

  /// Hot-path update: the queue processor receives the latest status alongside
  /// every successful submit response. Writing it straight into the cached
  /// state avoids a separate `/status` round-trip.
  void updateFromServer(CheckinUserStatusDto status) {
    state = AsyncValue<CheckinUserStatusDto?>.data(status);
  }
}

final checkinStatusProvider =
    AsyncNotifierProvider<CheckinStatusNotifier, CheckinUserStatusDto?>(
  CheckinStatusNotifier.new,
);
