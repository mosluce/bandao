import 'package:app_settings/app_settings.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';

import '../../../core/api/models/location_ping.dart';
import '../../../l10n/app_localizations.dart';
import '../../checkin/state/location_permission_provider.dart';
import '../data/trajectory_stats.dart';
import '../state/trajectory_controller.dart';

/// "我的工作日記" — AppUser-facing trajectory surface. The whole point of
/// this screen, per the App Review 2.5.4 response, is that persistent
/// background location is a feature *for the user themselves*, not only
/// for their employer.
class TrajectoryScreen extends ConsumerWidget {
  const TrajectoryScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final permission = ref.watch(locationPermissionProvider);
    final state = ref.watch(trajectoryProvider);

    return Scaffold(
      appBar: AppBar(title: Text(l10n.trajectoryTitle)),
      body: SafeArea(
        child: permission.maybeWhen(
          data: (perm) {
            if (perm == LocationPermission.denied ||
                perm == LocationPermission.deniedForever) {
              return _PermissionPrimer(l10n: l10n);
            }
            return Column(
              children: [
                _DateDropdown(state: state),
                Expanded(child: _Body(l10n: l10n, state: state)),
              ],
            );
          },
          orElse: () => const Center(child: CircularProgressIndicator()),
        ),
      ),
    );
  }
}

class _DateDropdown extends ConsumerWidget {
  const _DateDropdown({required this.state});

  final AsyncValue<TrajectoryDayState> state;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final selected = state.valueOrNull?.selectedDate ?? _today();

    // today + previous 7 days (8 entries total, today first).
    final options = List<DateTime>.generate(
      8,
      (i) => _today().subtract(Duration(days: i)),
    );

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: DropdownButtonFormField<DateTime>(
        key: const ValueKey('trajectoryDateDropdown'),
        // CI pins Flutter 3.29.3 which only knows `value:`. The newer
        // `initialValue:` (3.33+) replaces it and emits a deprecation
        // warning on local 3.38, but using it breaks the CI analyze step.
        // ignore: deprecated_member_use
        value: options.firstWhere(
          (d) => _sameDay(d, selected),
          orElse: () => options.first,
        ),
        decoration: const InputDecoration(border: OutlineInputBorder()),
        onChanged: (next) {
          if (next == null) return;
          ref.read(trajectoryProvider.notifier).selectDate(next);
        },
        items: [
          for (final d in options)
            DropdownMenuItem<DateTime>(
              value: d,
              child: Text(_sameDay(d, _today()) ? l10n.trajectoryDateToday : _label(d)),
            ),
        ],
      ),
    );
  }

  static DateTime _today() {
    final n = DateTime.now();
    return DateTime(n.year, n.month, n.day);
  }

  static bool _sameDay(DateTime a, DateTime b) =>
      a.year == b.year && a.month == b.month && a.day == b.day;

  static String _label(DateTime d) =>
      '${d.month.toString().padLeft(2, '0')}/${d.day.toString().padLeft(2, '0')}';
}

class _Body extends StatelessWidget {
  const _Body({required this.l10n, required this.state});

  final AppLocalizations l10n;
  final AsyncValue<TrajectoryDayState> state;

  @override
  Widget build(BuildContext context) {
    return state.when(
      loading: () => const Center(child: CircularProgressIndicator()),
      error: (e, _) => Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Text(
            e.toString(),
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodyMedium,
          ),
        ),
      ),
      data: (day) {
        if (day.pings.isEmpty) {
          return Center(
            child: Text(
              l10n.trajectoryEmpty,
              style: Theme.of(context).textTheme.bodyLarge,
            ),
          );
        }
        return Column(
          children: [
            Expanded(child: _Map(pings: day.pings, attribution: l10n.trajectoryAttribution)),
            _StatsPanel(stats: day.stats, l10n: l10n),
          ],
        );
      },
    );
  }
}

class _Map extends StatelessWidget {
  const _Map({required this.pings, required this.attribution});

  final List<LocationPingDto> pings;
  final String attribution;

  @override
  Widget build(BuildContext context) {
    final sorted = [...pings]
      ..sort((a, b) => a.occurredAtClient.compareTo(b.occurredAtClient));
    final points = sorted
        .map((p) => LatLng(p.lat, p.lng))
        .toList(growable: false);
    final bounds = LatLngBounds.fromPoints(points);

    return FlutterMap(
      options: MapOptions(
        initialCameraFit: CameraFit.bounds(
          bounds: bounds,
          padding: const EdgeInsets.all(32),
        ),
        interactionOptions: const InteractionOptions(
          flags: InteractiveFlag.pinchZoom | InteractiveFlag.drag,
        ),
      ),
      children: [
        TileLayer(
          // CARTO Positron — free, OSM-attributed.
          urlTemplate:
              'https://basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png',
          retinaMode: true,
          userAgentPackageName: 'tw.no8.bandao',
        ),
        PolylineLayer(
          polylines: [
            Polyline(
              points: points,
              strokeWidth: 4,
              color: Theme.of(context).colorScheme.primary,
            ),
          ],
        ),
        MarkerLayer(
          markers: [
            Marker(
              point: points.first,
              width: 24,
              height: 24,
              child: _Dot(color: Theme.of(context).colorScheme.secondary),
            ),
            if (points.length > 1)
              Marker(
                point: points.last,
                width: 24,
                height: 24,
                child: _Dot(color: Theme.of(context).colorScheme.primary),
              ),
          ],
        ),
        RichAttributionWidget(
          attributions: [TextSourceAttribution(attribution)],
        ),
      ],
    );
  }
}

class _Dot extends StatelessWidget {
  const _Dot({required this.color});
  final Color color;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: color,
        shape: BoxShape.circle,
        border: Border.all(color: Colors.white, width: 3),
      ),
    );
  }
}

class _StatsPanel extends StatelessWidget {
  const _StatsPanel({required this.stats, required this.l10n});

  final TrajectoryStats stats;
  final AppLocalizations l10n;

  @override
  Widget build(BuildContext context) {
    final km = stats.distanceMeters / 1000;
    final h = stats.onShiftDuration.inHours;
    final m = stats.onShiftDuration.inMinutes % 60;
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
        children: [
          _StatColumn(
            label: l10n.trajectoryStatDistance,
            value: l10n.trajectoryDistanceKm(km),
          ),
          _StatColumn(
            label: l10n.trajectoryStatDuration,
            value: l10n.trajectoryDurationHm(h, m),
          ),
          _StatColumn(
            label: l10n.trajectoryStatPings,
            value: stats.pingCount.toString(),
          ),
        ],
      ),
    );
  }
}

class _StatColumn extends StatelessWidget {
  const _StatColumn({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(label, style: theme.textTheme.bodySmall),
        const SizedBox(height: 4),
        Text(value, style: theme.textTheme.titleMedium),
      ],
    );
  }
}

class _PermissionPrimer extends ConsumerWidget {
  const _PermissionPrimer({required this.l10n});
  final AppLocalizations l10n;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              l10n.trajectoryPermissionTitle,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            Text(l10n.trajectoryPermissionBody, textAlign: TextAlign.center),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: () => AppSettings.openAppSettings(),
              child: Text(l10n.trajectoryPermissionOpenSettings),
            ),
          ],
        ),
      ),
    );
  }
}
