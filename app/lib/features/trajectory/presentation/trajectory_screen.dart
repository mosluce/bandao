import 'package:app_settings/app_settings.dart';
import 'package:flutter/material.dart';
import 'package:flutter_map/flutter_map.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:geolocator/geolocator.dart';
import 'package:latlong2/latlong.dart';

import '../../../core/api/models/checkin_event.dart';
import '../../../l10n/app_localizations.dart';
import '../../checkin/state/location_permission_provider.dart';
import '../data/time_of_day_color.dart';
import '../data/trajectory_merge.dart';
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
              child: Text(
                _sameDay(d, _today()) ? l10n.trajectoryDateToday : _label(d),
              ),
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
        // Rendering keys off the merged point count: 0 → the empty text and no
        // map at all; 1 → map plus markers; 2+ → map, markers, and the line.
        if (day.mergedSeries.isEmpty) {
          return Center(
            child: Text(
              l10n.trajectoryEmpty,
              style: Theme.of(context).textTheme.bodyLarge,
            ),
          );
        }
        return Column(
          children: [
            Expanded(
              child: _Map(
                mergedSeries: day.mergedSeries,
                events: day.events,
                attribution: l10n.trajectoryAttribution,
              ),
            ),
            _StatsPanel(stats: day.stats, l10n: l10n),
          ],
        );
      },
    );
  }
}

/// Distinct coordinates the camera must keep in view: the polyline's vertices
/// plus every event marker, **deduplicated**.
///
/// Deduplication is load-bearing, not tidiness. Since the merged series
/// carries the checkin coordinates, an event contributes the same `LatLng`
/// twice — once as a vertex, once as a marker. A day with a clock-in and no
/// pings yet therefore holds two entries at one place, and fitting a
/// zero-area bounds makes flutter_map compute an infinite zoom, which
/// `TileLayer` throws on: `UnsupportedError: Infinity or NaN toInt`.
///
/// That is the state every shift passes through between clocking in and the
/// first ping clearing the 100m/60s filter, so it is a path every user hits.
@visibleForTesting
List<LatLng> cameraPointsFor(
  List<MergedPoint> series,
  List<CheckinEventDto> events,
) {
  return <LatLng>{
    ...series.map((p) => LatLng(p.lat, p.lng)),
    ...events.map(
      (e) => LatLng(e.location.coordinates.lat, e.location.coordinates.lng),
    ),
  }.toList();
}

class _Map extends StatelessWidget {
  const _Map({
    required this.mergedSeries,
    required this.events,
    required this.attribution,
  });

  /// Already ordered by the merge contract — do not re-sort.
  final List<MergedPoint> mergedSeries;
  final List<CheckinEventDto> events;
  final String attribution;

  @override
  Widget build(BuildContext context) {
    final points = mergedSeries.map((p) => LatLng(p.lat, p.lng)).toList();
    final times = mergedSeries
        .map(
          (p) =>
              DateTime.tryParse(p.occurredAtClient)?.toLocal() ??
              DateTime.now(),
        )
        .toList();

    // One polyline per consecutive pair, colored by the segment's midpoint
    // time (flutter_map's gradientColors is a straight first→last projection,
    // so it can't follow a winding path — segments are the accurate approach).
    final segments = <Polyline>[
      for (var i = 0; i < points.length - 1; i++)
        Polyline(
          points: [points[i], points[i + 1]],
          strokeWidth: 4,
          color: timeOfDayColorForMinute(
            (_minuteOfDay(times[i]) + _minuteOfDay(times[i + 1])) ~/ 2,
          ),
        ),
    ];

    // Event markers (clock in/out, transfer in/out), colored by type. Drawn
    // from the FULL event list, not the merged series: an `admin_force`
    // clock_out is excluded from the line (its location is copied, not
    // captured) but must still show where the admin closed the shift.
    final eventPoints = events
        .map(
          (e) => LatLng(e.location.coordinates.lat, e.location.coordinates.lng),
        )
        .toList();

    // Everything to keep in view: the path + the event markers.
    //
    // Deduplicated, and the camera decision keys off DISTINCT coordinates
    // rather than list length. Since the merged series carries the checkin
    // coordinates, an event contributes the same LatLng twice — once as a
    // vertex, once as a marker. A day with a clock-in and no pings yet
    // therefore has two entries at one place, and fitting a zero-area bounds
    // makes flutter_map compute an infinite zoom:
    //   `UnsupportedError: Infinity or NaN toInt` from TileLayer.
    // That is the state every shift passes through between clocking in and
    // the first ping clearing the 100m/60s filter.
    final allPoints = cameraPointsFor(mergedSeries, events);

    return Stack(
      children: [
        FlutterMap(
          options: MapOptions(
            initialCameraFit: allPoints.length > 1
                ? CameraFit.bounds(
                    bounds: LatLngBounds.fromPoints(allPoints),
                    padding: const EdgeInsets.all(32),
                  )
                : null,
            initialCenter:
                allPoints.length == 1 ? allPoints.first : const LatLng(0, 0),
            initialZoom: allPoints.length == 1 ? 16 : 3,
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
            if (segments.isNotEmpty) PolylineLayer(polylines: segments),
            MarkerLayer(
              markers: [
                for (var i = 0; i < events.length; i++)
                  Marker(
                    point: eventPoints[i],
                    width: 22,
                    height: 22,
                    child: _Dot(color: _eventColor(events[i].eventType)),
                  ),
              ],
            ),
            RichAttributionWidget(
              attributions: [TextSourceAttribution(attribution)],
            ),
          ],
        ),
        const Positioned(
          left: 12,
          bottom: 12,
          child: _TimeLegend(),
        ),
      ],
    );
  }

  static int _minuteOfDay(DateTime t) => t.hour * 60 + t.minute;

  /// Event-type marker colors — matches admin-web's trajectory markers.
  static Color _eventColor(CheckinEventType type) {
    switch (type) {
      case CheckinEventType.clockIn:
        return const Color(0xFF15803D); // green — start of day
      case CheckinEventType.clockOut:
        return const Color(0xFF475569); // slate
      case CheckinEventType.transferIn:
      case CheckinEventType.transferOut:
        return const Color(0xFFB45309); // amber
    }
  }
}

/// "Color → time" legend: a horizontal warm→cool gradient bar with clock
/// labels, so a viewer can decode the path colors.
class _TimeLegend extends StatelessWidget {
  const _TimeLegend();

  @override
  Widget build(BuildContext context) {
    // Sample the scale at each hour across the domain for a smooth bar.
    final stops = <Color>[
      for (var h = 6; h <= 22; h++) timeOfDayColorForMinute(h * 60),
    ];
    return DecoratedBox(
      decoration: BoxDecoration(
        color: Colors.white.withValues(alpha: 0.9),
        borderRadius: BorderRadius.circular(8),
        boxShadow: const [
          BoxShadow(color: Colors.black26, blurRadius: 4, offset: Offset(0, 1)),
        ],
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              width: 160,
              height: 8,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(4),
                gradient: LinearGradient(colors: stops),
              ),
            ),
            const SizedBox(height: 2),
            const SizedBox(
              width: 160,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text('6:00', style: TextStyle(fontSize: 10)),
                  Text('12:00', style: TextStyle(fontSize: 10)),
                  Text('18:00', style: TextStyle(fontSize: 10)),
                  Text('22:00', style: TextStyle(fontSize: 10)),
                ],
              ),
            ),
          ],
        ),
      ),
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
            value: stats.pointCount.toString(),
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
