import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../core/api/models/checkin_status.dart';
import '../../../core/storage/secure_storage.dart';
import '../../../l10n/app_localizations.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../data/checkin_actions.dart';
import '../state/effective_status_provider.dart';
import '../state/location_permission_provider.dart';
import 'location_consent_dialog.dart';

class HomeButtons extends ConsumerWidget {
  const HomeButtons({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final eff = ref.watch(effectiveStatusProvider);
    final permission = ref.watch(locationPermissionProvider).valueOrNull;
    // Only `deniedForever` truly blocks the tap. `denied` includes the
    // "never asked yet" iOS state — we want the button enabled so first-tap
    // triggers the system permission dialog.
    final disabled = permission == LocationPermission.deniedForever;

    // When the org has disabled transfers, drop `[轉出]` / `[轉入]` from the
    // visible button set. Server has a state-lock that prevents flipping
    // this while anyone is non-off_duty, but the cached value can still be
    // stale; resume-refresh closes that window.
    final auth = ref.watch(authProvider).valueOrNull;
    final transferEnabled = auth is AuthAuthenticated
        ? auth.org.checkin.transferEnabled
        : true;

    final children = <Widget>[];
    switch (eff.status) {
      case AppUserCheckinStatus.offDuty:
        children.add(_button(
          context: context,
          label: l10n.eventClockIn,
          eventType: CheckinEventType.clockIn,
          disabled: disabled,
          ref: ref,
          primary: true,
        ),);
        break;
      case AppUserCheckinStatus.onSite:
        children.add(_button(
          context: context,
          label: l10n.eventClockOut,
          eventType: CheckinEventType.clockOut,
          disabled: disabled,
          ref: ref,
          primary: !transferEnabled,
        ),);
        if (transferEnabled) {
          children.add(const SizedBox(height: 12));
          children.add(_button(
            context: context,
            label: l10n.eventTransferOut,
            eventType: CheckinEventType.transferOut,
            disabled: disabled,
            ref: ref,
          ),);
        }
        break;
      case AppUserCheckinStatus.inTransit:
        children.add(_button(
          context: context,
          label: l10n.eventClockOut,
          eventType: CheckinEventType.clockOut,
          disabled: disabled,
          ref: ref,
          primary: !transferEnabled,
        ),);
        if (transferEnabled) {
          children.add(const SizedBox(height: 12));
          children.add(_button(
            context: context,
            label: l10n.eventTransferIn,
            eventType: CheckinEventType.transferIn,
            disabled: disabled,
            ref: ref,
            primary: true,
          ),);
        }
        break;
    }

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
  }

  Widget _button({
    required BuildContext context,
    required String label,
    required CheckinEventType eventType,
    required bool disabled,
    required WidgetRef ref,
    bool primary = false,
  }) {
    final onPressed = disabled
        ? null
        : () async {
            // Location-tracking consent gate: when the org has tracking
            // enabled and this is a `clock_in`, prompt the worker once
            // before the first ping is enqueued. Cancel aborts the shift
            // start entirely (no event enqueued).
            if (eventType == CheckinEventType.clockIn) {
              final consented = await _ensureLocationConsent(context, ref);
              if (!consented) return;
              if (!context.mounted) return;
            }
            final actions = ref.read(checkinActionsProvider);
            final outcome = await actions.enqueueEvent(eventType);
            if (!context.mounted) return;
            switch (outcome) {
              case EnqueueOutcome.enqueued:
                break;
              case EnqueueOutcome.locationUnavailable:
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(
                    content:
                        Text(AppLocalizations.of(context).locationUnavailable),
                  ),
                );
                break;
              case EnqueueOutcome.permissionDenied:
                // Blocker widget renders; nothing else to do.
                break;
              case EnqueueOutcome.notAuthenticated:
                break;
            }
          };

    return _renderButton(label: label, onPressed: onPressed, primary: primary);
  }

  Widget _renderButton({
    required String label,
    required VoidCallback? onPressed,
    required bool primary,
  }) {
    final size = const Size.fromHeight(56);
    return SizedBox(
      width: double.infinity,
      child: primary
          ? FilledButton(
              onPressed: onPressed,
              style: FilledButton.styleFrom(minimumSize: size),
              child: Text(label),
            )
          : OutlinedButton(
              onPressed: onPressed,
              style: OutlinedButton.styleFrom(minimumSize: size),
              child: Text(label),
            ),
    );
  }
}

/// Returns `true` when the user may proceed with `clock_in`, `false` if
/// they cancelled. Skips the dialog when:
///   - Org `location_tracking_enabled` is false (no tracking, no consent
///     needed)
///   - User has previously consented for this AppUser id
Future<bool> _ensureLocationConsent(
  BuildContext context,
  WidgetRef ref,
) async {
  final auth = ref.read(authProvider).valueOrNull;
  if (auth is! AuthAuthenticated) return true; // shouldn't happen on home
  if (!auth.org.checkin.transferEnabled &&
      !auth.org.checkin.locationTrackingEnabled) {
    return true;
  }
  if (!auth.org.checkin.locationTrackingEnabled) return true;

  final storage = ref.read(secureStorageProvider);
  final already = await storage.readLocationTrackingConsent(auth.user.id);
  if (already) return true;
  if (!context.mounted) return false;

  final agreed = await showLocationConsentDialog(context, ref);
  if (!agreed) return false;
  await storage.writeLocationTrackingConsent(auth.user.id);
  return true;
}
