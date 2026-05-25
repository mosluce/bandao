import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../l10n/app_localizations.dart';
import '../../checkin/presentation/background_sync_tip.dart';
import '../../checkin/presentation/home_buttons.dart';
import '../../checkin/presentation/location_permission_blocker.dart';
import '../../checkin/presentation/queue_chip.dart';
import '../../checkin/presentation/status_pill.dart';
import '../../checkin/presentation/tracking_chip.dart';
import '../../checkin/presentation/tracking_recovery_banner.dart';
import '../../checkin/state/checkin_queue_provider.dart';
import '../../checkin/state/checkin_status_provider.dart';
import '../../checkin/state/handover_notice_provider.dart';
import '../../checkin/state/location_permission_provider.dart';
import '../state/auth_provider.dart';
import '../state/auth_state.dart';

class HomeScreen extends ConsumerStatefulWidget {
  const HomeScreen({super.key});

  @override
  ConsumerState<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends ConsumerState<HomeScreen>
    with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    // Catch the case where the handover wipe ran during `_fetchMe` BEFORE
    // home was mounted. In that path the regular `ref.listen` below never
    // fires (Riverpod's WidgetRef.listen doesn't support fireImmediately),
    // so we read once on the first frame and surface the toast manually.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final pending = ref.read(pendingHandoverNoticeProvider);
      if (pending != null && pending.isNotEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(pending)),
        );
        ref.read(pendingHandoverNoticeProvider.notifier).state = null;
      }
    });
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      // The user might have toggled OS settings while we were in background.
      ref.read(locationPermissionProvider.notifier).refresh();
      // Refresh cached identity (org settings — esp. transferEnabled — and
      // user fields) and the server-confirmed checkin status. Resume is NOT
      // a login event, so the handover wipe does NOT run from this path.
      ref.read(authProvider.notifier).refreshMe();
      ref.read(checkinStatusProvider.notifier).refresh();
    }
  }

  Future<void> _onLogoutSelected() async {
    if (!mounted) return;
    final l10n = AppLocalizations.of(context);
    final queue = ref.read(checkinQueueProvider).valueOrNull ?? const [];
    // Inclusive count: pending + sending + failed all get wiped on a
    // different-user login per the existing handover semantics, so the
    // warning needs to reflect actual data-loss surface.
    final unsynced = queue.length;
    if (unsynced == 0) {
      await ref.read(authProvider.notifier).logout();
      return;
    }
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        title: Text(l10n.logoutConfirmTitle),
        content: Text(l10n.logoutConfirmBody(unsynced)),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.logoutConfirmCancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.logoutConfirmProceed),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    if (!mounted) return;
    await ref.read(authProvider.notifier).logout();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final auth = ref.watch(authProvider);
    final state = auth.value;
    final AuthAuthenticated? authed =
        state is AuthAuthenticated ? state : null;

    // Surface the handover notice when it arrives via the manual-login
    // microtask path (state flip → home mounts → wipe runs). The auto-login
    // path is covered by the postFrame check in `initState` since the value
    // is already set by the time home mounts.
    ref.listen<String?>(pendingHandoverNoticeProvider, (prev, next) {
      if (next == null || next.isEmpty) return;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(next)),
        );
        ref.read(pendingHandoverNoticeProvider.notifier).state = null;
      });
    });

    return Scaffold(
      appBar: AppBar(
        title: Text(authed?.org.name ?? ''),
        actions: <Widget>[
          PopupMenuButton<String>(
            onSelected: (v) {
              if (v == 'logout') {
                _onLogoutSelected();
              }
            },
            itemBuilder: (BuildContext context) => <PopupMenuEntry<String>>[
              PopupMenuItem<String>(
                value: 'logout',
                child: Text(l10n.homeLogout),
              ),
            ],
          ),
        ],
      ),
      body: authed == null
          ? const SafeArea(child: Center(child: CircularProgressIndicator()))
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                const TrackingRecoveryBanner(),
                Expanded(
                  child: SafeArea(
                    top: false,
                    child: SingleChildScrollView(
                      padding: const EdgeInsets.all(24),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            authed.user.displayName,
                            style: Theme.of(context).textTheme.headlineSmall,
                          ),
                          const SizedBox(height: 4),
                          Text(
                            authed.user.username,
                            style: Theme.of(context)
                                .textTheme
                                .bodyMedium
                                ?.copyWith(fontFamily: 'monospace'),
                          ),
                          const SizedBox(height: 24),
                          const CheckinStatusPill(),
                          const SizedBox(height: 16),
                          const BackgroundSyncTip(),
                          const SizedBox(height: 8),
                          const LocationPermissionBlocker(),
                          const SizedBox(height: 16),
                          const HomeButtons(),
                          const SizedBox(height: 16),
                          Wrap(
                            spacing: 8,
                            runSpacing: 8,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: <Widget>[
                              const QueueChip(),
                              const TrackingChip(),
                            ],
                          ),
                          // 「事件歷史」 entry moved to the persistent bottom
                          // NavigationBar shell; the in-page TextButton was
                          // removed to keep one canonical way to reach it.
                        ],
                      ),
                    ),
                  ),
                ),
              ],
            ),
    );
  }
}
