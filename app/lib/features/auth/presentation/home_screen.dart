import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../l10n/app_localizations.dart';
import '../../../shared/widgets/checkin_status_pill.dart';
import '../state/auth_provider.dart';
import '../state/auth_state.dart';

class HomeScreen extends ConsumerWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final auth = ref.watch(authProvider);
    final state = auth.value;

    final AuthAuthenticated? authed =
        state is AuthAuthenticated ? state : null;

    return Scaffold(
      appBar: AppBar(
        title: Text(authed?.org.name ?? ''),
        actions: <Widget>[
          PopupMenuButton<String>(
            onSelected: (v) {
              if (v == 'logout') {
                ref.read(authProvider.notifier).logout();
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
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: authed == null
              ? const Center(child: CircularProgressIndicator())
              : Column(
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
                  ],
                ),
        ),
      ),
    );
  }
}
