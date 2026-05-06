import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';

import '../../../core/api/api_error.dart';
import '../../../core/api/models/checkin_event.dart';
import '../../../l10n/app_localizations.dart';
import '../../auth/state/auth_provider.dart';
import '../../auth/state/auth_state.dart';
import '../data/checkin_queue_db.dart';
import '../data/checkin_repository.dart';
import '../state/checkin_queue_provider.dart';
import '../state/checkin_status_provider.dart';
import '../state/recently_synced_events_provider.dart';

class HistoryScreen extends ConsumerStatefulWidget {
  const HistoryScreen({super.key});

  @override
  ConsumerState<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends ConsumerState<HistoryScreen> {
  final List<CheckinEventDto> _serverEvents = <CheckinEventDto>[];
  bool _loading = false;
  bool _hasMore = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _loadMore());
  }

  Future<void> _loadMore() async {
    if (_loading || !_hasMore) return;
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final repo = await ref.read(checkinRepositoryProvider.future);
      final before =
          _serverEvents.isEmpty ? null : _serverEvents.last.occurredAtClient;
      final batch = await repo.events(before: before);
      if (!mounted) return;
      setState(() {
        _serverEvents.addAll(batch);
        if (batch.length < 50) _hasMore = false;
        _loading = false;
      });
    } on ApiException catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.message;
        _loading = false;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  Future<void> _refresh() async {
    setState(() {
      _serverEvents.clear();
      _hasMore = true;
      _error = null;
    });
    ref.read(recentlySyncedEventsProvider.notifier).clear();
    // Refresh status alongside so the user sees fresh state from one gesture.
    unawaited(ref.read(checkinStatusProvider.notifier).refresh());
    await _loadMore();
  }

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    final queue = ref.watch(checkinQueueProvider).valueOrNull ?? const [];
    final recentlySynced = ref.watch(recentlySyncedEventsProvider);
    final auth = ref.watch(authProvider).valueOrNull;
    final timezone =
        auth is AuthAuthenticated ? auth.org.timezone : 'Asia/Taipei';

    final entries = _mergeAndSort(queue, _serverEvents, recentlySynced);

    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.historyTitle),
        leading: BackButton(
          onPressed: () => context.canPop() ? context.pop() : context.go('/'),
        ),
      ),
      body: SafeArea(
        child: RefreshIndicator(
          onRefresh: _refresh,
          child: entries.isEmpty && !_loading
              ? ListView(
                  // ListView (vs Center) so RefreshIndicator's pull works
                  // even when the list is empty.
                  padding: const EdgeInsets.all(16),
                  children: <Widget>[
                    const SizedBox(height: 80),
                    Center(child: Text(l10n.historyEmpty)),
                  ],
                )
              : ListView.separated(
                  padding: const EdgeInsets.all(16),
                  itemCount: entries.length + 1,
                  separatorBuilder: (_, __) => const SizedBox(height: 12),
                  itemBuilder: (context, index) {
                    if (index == entries.length) {
                      return _Footer(
                        loading: _loading,
                        hasMore: _hasMore,
                        error: _error,
                        onLoadMore: _loadMore,
                      );
                    }
                    final entry = entries[index];
                    return _HistoryRow(entry: entry, timezone: timezone);
                  },
                ),
        ),
      ),
    );
  }

  /// Merges three sources by `occurred_at_client desc` and dedupes server
  /// events by `id`. The recently-synced cache is only consulted for events
  /// not already in `_serverEvents` (server-fetched wins because it has the
  /// authoritative `region_name` after geocoding). Local queue rows have no
  /// server `id` so they don't participate in dedupe.
  List<_Entry> _mergeAndSort(
    List<QueueRow> queue,
    List<CheckinEventDto> server,
    List<CheckinEventDto> recentlySynced,
  ) {
    final list = <_Entry>[];
    for (final r in queue) {
      list.add(_Entry.local(r));
    }
    final serverIds = <String>{};
    for (final e in server) {
      serverIds.add(e.id);
      list.add(_Entry.server(e));
    }
    for (final e in recentlySynced) {
      if (serverIds.contains(e.id)) continue;
      list.add(_Entry.server(e));
    }
    list.sort(
      (a, b) => b.occurredAtClient.compareTo(a.occurredAtClient),
    );
    return list;
  }
}

class _Entry {
  _Entry.local(QueueRow this.local) : server = null;
  _Entry.server(CheckinEventDto this.server) : local = null;

  final QueueRow? local;
  final CheckinEventDto? server;

  String get occurredAtClient =>
      local?.occurredAtClient ?? server!.occurredAtClient;
}

class _HistoryRow extends ConsumerWidget {
  const _HistoryRow({required this.entry, required this.timezone});

  final _Entry entry;
  final String timezone;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final scheme = Theme.of(context).colorScheme;
    final isLocal = entry.local != null;
    final eventTypeStr =
        isLocal ? entry.local!.eventType : entry.server!.eventType.toJson();
    final eventLabel = _eventLabel(eventTypeStr, l10n);
    final time = _formatTime(entry.occurredAtClient);
    final status = isLocal ? entry.local!.status : 'synced';
    final (badgeLabel, badgeColor) = _badgeStyle(status, l10n, scheme);

    final locationLabel = isLocal
        ? '${entry.local!.lat.toStringAsFixed(4)}, ${entry.local!.lng.toStringAsFixed(4)}'
        : (entry.server!.location.regionName?.isNotEmpty ?? false
            ? entry.server!.location.regionName!
            : '${entry.server!.location.coordinates.lat.toStringAsFixed(4)}, '
                '${entry.server!.location.coordinates.lng.toStringAsFixed(4)}');

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.surfaceContainerHighest,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: scheme.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Container(
                padding:
                    const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                decoration: BoxDecoration(
                  color: badgeColor.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  badgeLabel,
                  style: TextStyle(
                    color: badgeColor,
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Text(
                eventLabel,
                style: Theme.of(context).textTheme.titleSmall,
              ),
              const Spacer(),
              Text(
                time,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(
            locationLabel,
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
          ),
          if (isLocal && entry.local!.status == 'failed') ...<Widget>[
            const SizedBox(height: 8),
            Text(
              l10n.friendlyErrorCode(
                entry.local!.lastErrorCode ?? '',
                entry.local!.lastErrorMessage ?? '',
              ),
              style: TextStyle(color: scheme.error),
            ),
            const SizedBox(height: 4),
            Row(
              children: <Widget>[
                TextButton.icon(
                  icon: const Icon(Icons.copy_outlined, size: 16),
                  onPressed: () => _copyDetails(context, ref, entry.local!),
                  label: Text(l10n.historyCopyDetails),
                ),
                const SizedBox(width: 8),
                TextButton.icon(
                  icon: const Icon(Icons.close, size: 16),
                  onPressed: () => _dismiss(ref, entry.local!.id),
                  label: Text(l10n.historyDismiss),
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  String _eventLabel(String wire, AppLocalizations l10n) {
    switch (wire) {
      case 'clock_in':
        return l10n.eventClockIn;
      case 'clock_out':
        return l10n.eventClockOut;
      case 'transfer_out':
        return l10n.eventTransferOut;
      case 'transfer_in':
        return l10n.eventTransferIn;
      default:
        return wire;
    }
  }

  String _formatTime(String rfc3339) {
    final t = DateTime.tryParse(rfc3339);
    if (t == null) return rfc3339;
    final local = t.toLocal();
    final hh = local.hour.toString().padLeft(2, '0');
    final mm = local.minute.toString().padLeft(2, '0');
    return '${local.month}/${local.day} $hh:$mm';
  }

  (String, Color) _badgeStyle(
    String status,
    AppLocalizations l10n,
    ColorScheme scheme,
  ) {
    switch (status) {
      case 'pending':
        return (l10n.badgePending, scheme.tertiary);
      case 'sending':
        return (l10n.badgeSending, scheme.primary);
      case 'failed':
        return (l10n.badgeFailed, scheme.error);
      case 'synced':
      default:
        return (l10n.badgeSynced, scheme.outline);
    }
  }

  Future<void> _copyDetails(
    BuildContext context,
    WidgetRef ref,
    QueueRow row,
  ) async {
    final blob = _detailBlob(row);
    await Clipboard.setData(ClipboardData(text: blob));
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(AppLocalizations.of(context).historyCopiedToast)),
    );
  }

  Future<void> _dismiss(WidgetRef ref, int id) async {
    final db = ref.read(checkinQueueDbProvider);
    await db.deleteRow(id);
  }

  String _detailBlob(QueueRow row) {
    final accuracy =
        row.accuracy == null ? '' : ' (±${row.accuracy!.toStringAsFixed(0)}m)';
    return [
      'Bandao checkin event report',
      'event_id: queue#${row.id}',
      'event_type: ${row.eventType}',
      'occurred_at_client: ${row.occurredAtClient}',
      'lat, lng: ${row.lat}, ${row.lng}$accuracy',
      'attempts: ${row.attempts}',
      'last_error_code: ${row.lastErrorCode ?? ""}',
      'last_error_message: ${row.lastErrorMessage ?? ""}',
      'last_attempt_at: ${row.lastAttemptAt ?? ""}',
      'app_user_id: ${row.appUserId}',
    ].join('\n');
  }
}

class _Footer extends StatelessWidget {
  const _Footer({
    required this.loading,
    required this.hasMore,
    required this.error,
    required this.onLoadMore,
  });

  final bool loading;
  final bool hasMore;
  final String? error;
  final VoidCallback onLoadMore;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context);
    if (loading) {
      return const Center(
        child: Padding(
          padding: EdgeInsets.all(16),
          child: CircularProgressIndicator(),
        ),
      );
    }
    if (error != null) {
      return Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: <Widget>[
            Text(error!, style: TextStyle(color: Theme.of(context).colorScheme.error)),
            const SizedBox(height: 8),
            OutlinedButton(
              onPressed: onLoadMore,
              child: Text(l10n.historyLoadMore),
            ),
          ],
        ),
      );
    }
    if (!hasMore) {
      return const SizedBox.shrink();
    }
    return Center(
      child: TextButton(
        onPressed: onLoadMore,
        child: Text(l10n.historyLoadMore),
      ),
    );
  }
}
