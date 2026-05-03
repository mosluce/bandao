import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../l10n/app_localizations.dart';
import '../state/auth_provider.dart';
import '../state/auth_state.dart';

/// Splash shown while we resolve the auto-login flow. Surfaces a retry
/// affordance when the bootstrap call hits a network error so the user
/// is not stranded with no way forward.
class SplashScreen extends ConsumerWidget {
  const SplashScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = AppLocalizations.of(context);
    final auth = ref.watch(authProvider);

    return Scaffold(
      body: Center(
        child: auth.maybeWhen(
          data: (state) {
            if (state is AuthError) {
              return _ErrorBlock(
                message: state.message.isEmpty
                    ? l10n.splashNetworkMessage
                    : state.message,
                onRetry: () => ref.read(authProvider.notifier).retry(),
                onLogout: () => ref.read(authProvider.notifier).logout(),
                retryLabel: l10n.splashRetry,
                logoutLabel: l10n.splashLogout,
              );
            }
            return const CircularProgressIndicator();
          },
          orElse: () => const CircularProgressIndicator(),
        ),
      ),
    );
  }
}

class _ErrorBlock extends StatelessWidget {
  const _ErrorBlock({
    required this.message,
    required this.onRetry,
    required this.onLogout,
    required this.retryLabel,
    required this.logoutLabel,
  });

  final String message;
  final VoidCallback onRetry;
  final VoidCallback onLogout;
  final String retryLabel;
  final String logoutLabel;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(24),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Text(
            message,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.bodyLarge,
          ),
          const SizedBox(height: 24),
          FilledButton(onPressed: onRetry, child: Text(retryLabel)),
          const SizedBox(height: 8),
          TextButton(onPressed: onLogout, child: Text(logoutLabel)),
        ],
      ),
    );
  }
}
